#[cfg(test)]
mod tests {
    use cairo_vm::air_private_input::{AirPrivateInput, AirPrivateInputSerializable};
    use cairo_vm::cairo_run::CairoRunConfig;
    use cairo_vm::hint_processor::builtin_hint_processor::bootloader::types::{
        BootloaderConfig, BootloaderInput, PackedOutput, SimpleBootloaderInput, Task, TaskSpec,
    };
    use cairo_vm::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor;
    use cairo_vm::hint_processor::hint_processor_definition::HintProcessor;
    use cairo_vm::types::program::Program;
    use cairo_vm::vm::errors::cairo_run_errors::CairoRunError;
    use cairo_vm::vm::errors::vm_exception::VmException;
    use cairo_vm::vm::runners::cairo_pie::CairoPie;
    use cairo_vm::vm::runners::cairo_runner::CairoRunner;
    use cairo_vm::vm::security::verify_secure_runner;
    use cairo_vm::vm::vm_core::VirtualMachine;
    use cairo_vm::{any_box, Felt252};
    use madara_prover_common::models::PublicInput;
    use madara_prover_common::toolkit::read_json_from_file;
    use madara_prover_rpc_server::cairo::{extract_execution_artifacts, ExecutionArtifacts};
    use rstest::{fixture, rstest};
    use std::any::Any;
    use std::collections::HashMap;
    use std::path::Path;
    use test_cases::{get_test_case_file_path, load_test_case_file};
    use test_fixtures::{assert_memory_eq, assert_private_input_eq};

    // Copied from cairo_run.rs and adapted to support injecting the bootloader input.
    // TODO: check if modifying CairoRunConfig to specify custom variables is accepted upstream.tcasm
    pub fn cairo_run(
        program: &Program,
        cairo_run_config: &CairoRunConfig,
        hint_executor: &mut dyn HintProcessor,
        variables: HashMap<String, Box<dyn Any>>,
    ) -> Result<(CairoRunner, VirtualMachine), CairoRunError> {
        let secure_run = cairo_run_config
            .secure_run
            .unwrap_or(!cairo_run_config.proof_mode);

        let mut cairo_runner = CairoRunner::new(
            &program,
            cairo_run_config.layout,
            cairo_run_config.proof_mode,
        )?;
        for (key, value) in variables {
            cairo_runner.exec_scopes.insert_box(&key, value);
        }

        let mut vm = VirtualMachine::new(cairo_run_config.trace_enabled);
        let end = cairo_runner.initialize(&mut vm)?;
        // check step calculation

        cairo_runner
            .run_until_pc(end, &mut vm, hint_executor)
            .map_err(|err| VmException::from_vm_error(&cairo_runner, &vm, err))?;
        cairo_runner.end_run(
            cairo_run_config.disable_trace_padding,
            false,
            &mut vm,
            hint_executor,
        )?;

        vm.verify_auto_deductions()?;
        cairo_runner.read_return_values(&mut vm)?;
        if cairo_run_config.proof_mode {
            cairo_runner.finalize_segments(&mut vm)?;
        }
        if secure_run {
            verify_secure_runner(&cairo_runner, true, None, &mut vm)?;
        }
        cairo_runner.relocate(&mut vm, cairo_run_config.relocate_mem)?;

        Ok((cairo_runner, vm))
    }

    pub fn run_bootloader_in_proof_mode(
        bootloader: &Program,
        tasks: Vec<TaskSpec>,
    ) -> Result<(CairoRunner, VirtualMachine), CairoRunError> {
        let proof_mode = true;
        let layout = "starknet_with_keccak";

        let cairo_run_config = CairoRunConfig {
            entrypoint: "main",
            trace_enabled: true,
            relocate_mem: true,
            layout,
            proof_mode,
            secure_run: None,
            disable_trace_padding: false,
        };

        let n_tasks = tasks.len();

        let bootloader_input = BootloaderInput {
            simple_bootloader_input: SimpleBootloaderInput {
                fact_topologies_path: None,
                single_page: false,
                tasks,
            },
            bootloader_config: BootloaderConfig {
                simple_bootloader_program_hash: Felt252::from(0),
                supported_cairo_verifier_program_hashes: vec![],
            },
            packed_outputs: vec![PackedOutput::Plain(vec![]); n_tasks],
        };

        let mut hint_processor = BuiltinHintProcessor::new_empty();
        let variables = HashMap::<String, Box<dyn Any>>::from([
            ("bootloader_input".to_string(), any_box!(bootloader_input)),
            (
                "bootloader_program".to_string(),
                any_box!(bootloader.clone()),
            ),
        ]);

        cairo_run(
            bootloader,
            &cairo_run_config,
            &mut hint_processor,
            variables,
        )
    }

    #[fixture]
    fn bootloader() -> Program {
        let bootloader = Program::from_file(
            get_test_case_file_path("bootloader/bootloader_compiled.json").as_path(),
            Some("main"),
        )
        .unwrap();

        bootloader
    }

    fn expected_output(test_case_dir: &Path) -> ExecutionArtifacts {
        let output_dir = test_case_dir.join("output");

        let public_input_file = output_dir.join("air_public_input.json");
        let private_input_file = output_dir.join("air_private_input.json");
        let memory_file = output_dir.join("memory.bin");
        let trace_file = output_dir.join("trace.bin");

        let public_input: PublicInput = read_json_from_file(public_input_file).unwrap();
        let private_input: AirPrivateInputSerializable =
            read_json_from_file(private_input_file).unwrap();
        let memory = std::fs::read(memory_file).unwrap();
        let trace = std::fs::read(trace_file).unwrap();

        ExecutionArtifacts {
            public_input,
            private_input: AirPrivateInput::from(private_input),
            memory,
            trace,
        }
    }

    #[rstest]
    #[case::fibonacci("fibonacci")]
    fn test_program(bootloader: Program, #[case] test_case: String) {
        let test_case_dir = get_test_case_file_path(&format!("bootloader/programs/{}", test_case));
        let expected_output = expected_output(&test_case_dir);

        let program_content =
            load_test_case_file(&format!("{}/program.json", test_case_dir.to_string_lossy()));
        let program = Program::from_bytes(program_content.as_bytes(), Some("main")).unwrap();
        let tasks = vec![TaskSpec {
            task: Task::Program(program),
        }];

        let (runner, vm) = run_bootloader_in_proof_mode(&bootloader, tasks).unwrap();
        let artifacts = extract_execution_artifacts(runner, vm).unwrap();

        assert_eq!(artifacts.public_input, expected_output.public_input);
        assert_eq!(artifacts.trace, expected_output.trace);

        assert_private_input_eq(artifacts.private_input, expected_output.private_input);
        assert_memory_eq(&artifacts.memory, &expected_output.memory);
    }

    #[rstest]
    #[case::fibonacci("fibonacci")]
    fn test_cairo_pie(bootloader: Program, #[case] test_case: String) {
        let test_case_dir = get_test_case_file_path(&format!("bootloader/pies/{}", test_case));
        let expected_output = expected_output(&test_case_dir);
        let cairo_pie_path = get_test_case_file_path(&format!(
            "{}/cairo_pie.zip",
            test_case_dir.to_string_lossy()
        ));

        let cairo_pie = CairoPie::from_file(cairo_pie_path.as_path()).unwrap();
        let tasks = vec![TaskSpec {
            task: Task::Pie(cairo_pie),
        }];

        let (runner, vm) = run_bootloader_in_proof_mode(&bootloader, tasks).unwrap();
        let artifacts = extract_execution_artifacts(runner, vm).unwrap();

        assert_eq!(artifacts.public_input, expected_output.public_input);
        assert_eq!(artifacts.trace, expected_output.trace);

        assert_private_input_eq(artifacts.private_input, expected_output.private_input);
        assert_memory_eq(&artifacts.memory, &expected_output.memory);
    }

    #[rstest]
    fn test_os_pie(bootloader: Program) {
        let os_pie_path = get_test_case_file_path("starknet-os/os.zip");

        let os_pie = CairoPie::from_file(os_pie_path.as_path()).unwrap();
        let tasks = vec![TaskSpec {
            task: Task::Pie(os_pie),
        }];

        let (runner, vm) = run_bootloader_in_proof_mode(&bootloader, tasks).unwrap();
        let artifacts = extract_execution_artifacts(runner, vm).unwrap();
        println!("{:?}", artifacts.public_input);
    }
}
