#[cfg(test)]
mod tests {
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
    use madara_prover_rpc_server::cairo::extract_execution_artifacts;
    use rstest::{fixture, rstest};
    use std::any::Any;
    use std::collections::HashMap;
    use std::path::Path;
    use test_cases::get_test_case_file_path;

    // Copied from cairo_run.rs and adapted to support injecting the bootloader input.
    // TODO: check if modifying CairoRunConfig to specify custom variables is accepted upstream.
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

    #[rstest]
    fn test_program(bootloader: Program) {
        // let program_content = load_test_case_file("fibonacci/fibonacci_compiled.json");
        // let program_content = load_test_case_file("hello-world/hello_world_compiled.json");
        let program_content = std::fs::read_to_string(Path::new(
            "../../starkware/cairo-vm/cairo_programs/fibonacci.json",
        ))
        .expect("Failed to read the fixture file");

        let program = Program::from_bytes(program_content.as_bytes(), Some("main")).unwrap();
        let tasks = vec![TaskSpec {
            task: Task::Program(program),
        }];

        let (runner, vm) = run_bootloader_in_proof_mode(&bootloader, tasks).unwrap();
        let artifacts = extract_execution_artifacts(runner, vm).unwrap();
        println!("{:?}", artifacts.public_input);
    }

    #[rstest]
    fn test_cairo_pie(bootloader: Program) {
        let cairo_pie_path = Path::new("/home/olivier/git/moonsong-labs/starkware/cairo-vm/cairo_programs/manually_compiled/fibonacci_cairo_pie/fibonacci_pie.zip");
        // let cairo_pie_path = Path::new(
        //     "/home/olivier/git/moonsong-labs/starkware/cairo-lang/fibonacci_no_builtin_pie.zip",
        // );

        let cairo_pie = CairoPie::from_file(cairo_pie_path).unwrap();
        let tasks = vec![TaskSpec {
            task: Task::Pie(cairo_pie),
        }];

        let (runner, vm) = run_bootloader_in_proof_mode(&bootloader, tasks).unwrap();
        let artifacts = extract_execution_artifacts(runner, vm).unwrap();
        println!("{:?}", artifacts.public_input);
    }

    // #[test]
    // fn test_sanity_check() {
    //     let cairo_run_config = CairoRunConfig {
    //         entrypoint: "main",
    //         trace_enabled: true,
    //         relocate_mem: true,
    //         layout: "starknet_with_keccak",
    //         proof_mode: true,
    //         secure_run: None,
    //         disable_trace_padding: false,
    //     };
    //
    //     let program_content = load_test_case_file("fibonacci/fibonacci_compiled.json");
    //     let mut hint_processor = BuiltinHintProcessor::new_empty();
    //
    //     cairo_run(
    //         program_content.as_bytes(),
    //         &cairo_run_config,
    //         &mut hint_processor,
    //         HashMap::new(),
    //     )
    //     .unwrap();
    // }
}
