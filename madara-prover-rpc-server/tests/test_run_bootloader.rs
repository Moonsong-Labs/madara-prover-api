#[cfg(test)]
mod tests {
    use std::path::Path;

    use cairo_vm::air_private_input::{AirPrivateInput, AirPrivateInputSerializable};
    use cairo_vm::hint_processor::builtin_hint_processor::bootloader::types::{Task, TaskSpec};
    use cairo_vm::types::program::Program;
    use cairo_vm::vm::runners::cairo_pie::CairoPie;
    use rstest::{fixture, rstest};
    use stone_prover_sdk::json::read_json_from_file;
    use stone_prover_sdk::models::PublicInput;

    use madara_prover_rpc_server::services::starknet_prover::run_bootloader_in_proof_mode;
    use stone_prover_sdk::cairo_vm::ExecutionArtifacts;
    use test_cases::{get_test_case_file_path, load_test_case_file};
    use test_fixtures::{assert_memory_eq, assert_private_input_eq};

    #[fixture]
    fn bootloader() -> Program {
        let bootloader = Program::from_file(
            get_test_case_file_path("bootloader/bootloader.json").as_path(),
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

        let artifacts = run_bootloader_in_proof_mode(&bootloader, tasks).unwrap();

        assert_eq!(artifacts.public_input, expected_output.public_input);
        assert_eq!(artifacts.trace, expected_output.trace);

        assert_private_input_eq(artifacts.private_input, expected_output.private_input);
        assert_memory_eq(&artifacts.memory, &expected_output.memory);
    }

    #[rstest]
    #[case::fibonacci("fibonacci")]
    #[case::fibonacci_stone_e2e("fibonacci-stone-e2e")]
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

        let artifacts = run_bootloader_in_proof_mode(&bootloader, tasks).unwrap();

        assert_eq!(artifacts.public_input, expected_output.public_input);
        assert_eq!(artifacts.trace, expected_output.trace);

        assert_private_input_eq(artifacts.private_input, expected_output.private_input);
        assert_memory_eq(&artifacts.memory, &expected_output.memory);
    }

    #[rstest]
    fn test_os_pie(bootloader: Program) {
        let test_case_dir = get_test_case_file_path("starknet-os");
        let expected_output = expected_output(&test_case_dir);

        let os_pie_path = get_test_case_file_path("starknet-os/os.zip");

        let os_pie = CairoPie::from_file(os_pie_path.as_path()).unwrap();
        let tasks = vec![TaskSpec {
            task: Task::Pie(os_pie),
        }];

        let artifacts = run_bootloader_in_proof_mode(&bootloader, tasks).unwrap();

        assert_eq!(artifacts.public_input, expected_output.public_input);
        assert_eq!(artifacts.trace, expected_output.trace);

        assert_private_input_eq(artifacts.private_input, expected_output.private_input);
        assert_memory_eq(&artifacts.memory, &expected_output.memory);
    }
}
