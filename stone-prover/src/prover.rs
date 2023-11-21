use std::path::{Path, PathBuf};

use tempfile::tempdir;

use crate::error::ProverError;
use crate::models::{PrivateInput, Proof, ProverConfig, ProverParameters, PublicInput};
use crate::toolkit::{read_json_from_file, write_json_to_file};

/// Call the Stone Prover from the command line.
///
/// Input files must be prepared by the caller.
///
/// * `public_input_file`: Path to the public input file.
/// * `private_input_file`: Path to the private input file. The private input file points to
///                         the memory and trace files.
/// * `prover_config_file`: Path to the prover configuration file. Contains application-agnostic
///                         configuration values for the prover.
/// * `parameter_file`: Path to the prover parameters file. Contains application-specific
///                     configuration values for the prover (ex: FRI steps).
/// * `output_file`: Path to the proof file. This function will write the generated proof
///                  as JSON to this file.
pub fn run_prover_from_command_line(
    public_input_file: &Path,
    private_input_file: &Path,
    prover_config_file: &Path,
    prover_parameter_file: &Path,
    output_file: &Path,
) -> Result<(), ProverError> {
    let output = std::process::Command::new("cpu_air_prover")
        .arg("--out-file")
        .arg(output_file)
        .arg("--public-input-file")
        .arg(public_input_file)
        .arg("--private-input-file")
        .arg(private_input_file)
        .arg("--prover-config-file")
        .arg(prover_config_file)
        .arg("--parameter-file")
        .arg(prover_parameter_file)
        .output()?;

    if !output.status.success() {
        return Err(ProverError::CommandError(output));
    }

    Ok(())
}

/// Call the Stone Prover from the command line, asynchronously.
///
/// Input files must be prepared by the caller.
///
/// * `public_input_file`: Path to the public input file.
/// * `private_input_file`: Path to the private input file. The private input file points to
///                         the memory and trace files.
/// * `prover_config_file`: Path to the prover configuration file. Contains application-agnostic
///                         configuration values for the prover.
/// * `parameter_file`: Path to the prover parameters file. Contains application-specific
///                     configuration values for the prover (ex: FRI steps).
/// * `output_file`: Path to the proof file. This function will write the generated proof
///                  as JSON to this file.
pub async fn run_prover_from_command_line_async(
    public_input_file: &Path,
    private_input_file: &Path,
    prover_config_file: &Path,
    parameter_file: &Path,
    output_file: &Path,
) -> Result<(), ProverError> {
    let output = tokio::process::Command::new("cpu_air_prover")
        .arg("--out-file")
        .arg(output_file)
        .arg("--public-input-file")
        .arg(public_input_file)
        .arg("--private-input-file")
        .arg(private_input_file)
        .arg("--prover-config-file")
        .arg(prover_config_file)
        .arg("--parameter-file")
        .arg(parameter_file)
        .output()
        .await?;

    if !output.status.success() {
        return Err(ProverError::CommandError(output));
    }

    Ok(())
}

struct ProverWorkingDirectory {
    _dir: tempfile::TempDir,
    public_input_file: PathBuf,
    private_input_file: PathBuf,
    _memory_file: PathBuf,
    _trace_file: PathBuf,
    prover_config_file: PathBuf,
    prover_parameter_file: PathBuf,
    proof_file: PathBuf,
}

fn prepare_prover_files(
    public_input: &PublicInput,
    memory: &Vec<u8>,
    trace: &Vec<u8>,
    prover_config: &ProverConfig,
    parameters: &ProverParameters,
) -> Result<ProverWorkingDirectory, std::io::Error> {
    let tmp_dir = tempdir()?;

    let tmp_dir_path = tmp_dir.path();

    let public_input_file = tmp_dir_path.join("public_input.json");
    let private_input_file = tmp_dir_path.join("private_input.json");
    let memory_file = tmp_dir_path.join("memory.bin");
    let prover_config_file = tmp_dir_path.join("prover_config_file.json");
    let prover_parameter_file = tmp_dir_path.join("parameters.json");
    let trace_file = tmp_dir_path.join("trace.bin");
    let proof_file = tmp_dir_path.join("proof.json");

    // Write public input and config/parameters files
    write_json_to_file(public_input, &public_input_file)?;
    write_json_to_file(prover_config, &prover_config_file)?;
    write_json_to_file(parameters, &prover_parameter_file)?;

    // Write memory and trace files
    std::fs::write(&memory_file, memory)?;
    std::fs::write(&trace_file, trace)?;

    // Write private input file
    let private_input = PrivateInput {
        memory_path: memory_file.clone(),
        trace_path: trace_file.clone(),
        pedersen: vec![],
        range_check: vec![],
        ecdsa: vec![],
    };

    write_json_to_file(private_input, &private_input_file)?;

    Ok(ProverWorkingDirectory {
        _dir: tmp_dir,
        public_input_file,
        private_input_file,
        _memory_file: memory_file,
        _trace_file: trace_file,
        prover_config_file,
        prover_parameter_file,
        proof_file,
    })
}

/// Run the Stone Prover on the specified program execution.
///
/// This function abstracts the method used to call the prover. At the moment we invoke
/// the prover as a subprocess but other methods can be implemented (ex: FFI).
///
/// * `public_input`: the public prover input generated by the Cairo program.
/// * `memory`: the memory output of the Cairo program.
/// * `trace`: the execution trace of the Cairo program.
/// * `prover_config`: prover configuration.
/// * `parameters`: prover parameters for the Cairo program.
pub fn run_prover(
    public_input: &PublicInput,
    memory: &Vec<u8>,
    trace: &Vec<u8>,
    prover_config: &ProverConfig,
    parameters: &ProverParameters,
) -> Result<Proof, ProverError> {
    let prover_working_dir =
        prepare_prover_files(public_input, memory, trace, prover_config, parameters)?;

    // Call the prover
    run_prover_from_command_line(
        &prover_working_dir.public_input_file,
        &prover_working_dir.private_input_file,
        &prover_working_dir.prover_config_file,
        &prover_working_dir.prover_parameter_file,
        &prover_working_dir.proof_file,
    )?;

    // Load the proof from the generated JSON proof file
    let proof = read_json_from_file(&prover_working_dir.proof_file)?;
    Ok(proof)
}

/// Run the Stone Prover on the specified program execution, asynchronously.
///
/// The main difference from the synchronous implementation is that the prover process
/// is spawned asynchronously using `tokio::process::Command`.
///
/// This function abstracts the method used to call the prover. At the moment we invoke
/// the prover as a subprocess but other methods can be implemented (ex: FFI).
///
/// * `public_input`: the public prover input generated by the Cairo program.
/// * `memory`: the memory output of the Cairo program.
/// * `trace`: the execution trace of the Cairo program.
/// * `prover_config`: prover configuration.
/// * `parameters`: prover parameters for the Cairo program.
pub async fn run_prover_async(
    public_input: &PublicInput,
    memory: &Vec<u8>,
    trace: &Vec<u8>,
    prover_config: &ProverConfig,
    parameters: &ProverParameters,
) -> Result<Proof, ProverError> {
    let prover_working_dir =
        prepare_prover_files(public_input, memory, trace, prover_config, parameters)?;

    // Call the prover
    run_prover_from_command_line_async(
        &prover_working_dir.public_input_file,
        &prover_working_dir.private_input_file,
        &prover_working_dir.prover_config_file,
        &prover_working_dir.prover_parameter_file,
        &prover_working_dir.proof_file,
    )
    .await?;

    // Load the proof from the generated JSON proof file
    let proof = read_json_from_file(&prover_working_dir.proof_file)?;
    Ok(proof)
}

#[cfg(test)]
mod test {
    use rstest::{fixture, rstest};
    use tempfile::NamedTempFile;

    use test_cases::get_fixture_path;

    use crate::models::{PrivateInput, Proof};
    use crate::toolkit::read_json_from_file;

    use super::*;

    /// Reads and deserializes a JSON proof file.
    fn read_proof_file<P: AsRef<Path>>(proof_file: P) -> Proof {
        let proof: Proof = read_json_from_file(proof_file).expect("Could not open proof file");
        proof
    }

    /// All the files forming a complete prover test case.
    struct ProverTestCase {
        public_input_file: PathBuf,
        prover_config_file: PathBuf,
        prover_parameter_file: PathBuf,
        memory_file: PathBuf,
        trace_file: PathBuf,
        proof_file: PathBuf,
    }

    #[fixture]
    fn fibonacci() -> ProverTestCase {
        let public_input_file = get_fixture_path("fibonacci/fibonacci_public_input.json");
        let prover_config_file = get_fixture_path("fibonacci/cpu_air_prover_config.json");
        let prover_parameter_file = get_fixture_path("fibonacci/cpu_air_params.json");
        let memory_file = get_fixture_path("fibonacci/fibonacci_memory.bin");
        let trace_file = get_fixture_path("fibonacci/fibonacci_trace.bin");
        let proof_file = get_fixture_path("fibonacci/fibonacci_proof.json");

        ProverTestCase {
            public_input_file,
            prover_config_file,
            prover_parameter_file,
            memory_file,
            trace_file,
            proof_file,
        }
    }

    /// Test case files adapted to match the prover command line arguments.
    struct ProverCliTestCase {
        public_input_file: PathBuf,
        private_input_file: NamedTempFile,
        prover_config_file: PathBuf,
        prover_parameter_file: PathBuf,
        proof: Proof,
    }

    #[fixture]
    fn prover_cli_test_case(#[from(fibonacci)] files: ProverTestCase) -> ProverCliTestCase {
        // Generate the private input in a temporary file
        let private_input_file =
            NamedTempFile::new().expect("Creating temporary private input file failed");
        let private_input = PrivateInput {
            memory_path: files.memory_file.clone(),
            trace_path: files.trace_file.clone(),
            pedersen: vec![],
            range_check: vec![],
            ecdsa: vec![],
        };

        serde_json::to_writer(&private_input_file, &private_input)
            .expect("Writing private input file failed");

        let proof = read_proof_file(&files.proof_file);

        ProverCliTestCase {
            public_input_file: files.public_input_file,
            private_input_file,
            prover_config_file: files.prover_config_file,
            prover_parameter_file: files.prover_parameter_file,
            proof,
        }
    }

    struct ParsedProverTestCase {
        public_input: PublicInput,
        memory: Vec<u8>,
        trace: Vec<u8>,
        prover_config: ProverConfig,
        prover_parameters: ProverParameters,
        proof: Proof,
    }

    #[fixture]
    fn parsed_prover_test_case(#[from(fibonacci)] files: ProverTestCase) -> ParsedProverTestCase {
        let public_input: PublicInput = read_json_from_file(files.public_input_file).unwrap();
        let prover_config: ProverConfig = read_json_from_file(files.prover_config_file).unwrap();
        let prover_parameters: ProverParameters =
            read_json_from_file(files.prover_parameter_file).unwrap();
        let memory = std::fs::read(files.memory_file).unwrap();
        let trace = std::fs::read(files.trace_file).unwrap();

        let proof = read_proof_file(&files.proof_file);

        ParsedProverTestCase {
            public_input,
            memory,
            trace,
            prover_config,
            prover_parameters,
            proof,
        }
    }

    #[fixture]
    fn prover_in_path() {
        // Add build dir to path for the duration of the test
        let path = std::env::var("PATH").unwrap_or_default();
        let build_dir = env!("OUT_DIR");
        std::env::set_var("PATH", format!("{build_dir}:{path}"));
    }

    /// Check that the Stone Prover command-line wrapper works.
    #[rstest]
    fn test_run_prover_from_command_line(
        prover_cli_test_case: ProverCliTestCase,
        #[from(prover_in_path)] _path: (),
    ) {
        // Add build dir to path for the duration of the test
        let path = std::env::var("PATH").unwrap_or_default();
        let build_dir = env!("OUT_DIR");
        std::env::set_var("PATH", format!("{build_dir}:{path}"));

        let output_file = NamedTempFile::new().expect("Creating output file failed");
        run_prover_from_command_line(
            &prover_cli_test_case.public_input_file,
            &prover_cli_test_case.private_input_file.path(),
            &prover_cli_test_case.prover_config_file,
            &prover_cli_test_case.prover_parameter_file,
            output_file.path(),
        )
        .unwrap();

        let proof = read_proof_file(output_file.path());
        assert_eq!(proof.proof_hex, prover_cli_test_case.proof.proof_hex);
    }

    #[rstest]
    fn test_run_prover(
        parsed_prover_test_case: ParsedProverTestCase,
        #[from(prover_in_path)] _path: (),
    ) {
        let proof = run_prover(
            &parsed_prover_test_case.public_input,
            &parsed_prover_test_case.memory,
            &parsed_prover_test_case.trace,
            &parsed_prover_test_case.prover_config,
            &parsed_prover_test_case.prover_parameters,
        )
        .unwrap();

        assert_eq!(proof.proof_hex, parsed_prover_test_case.proof.proof_hex);
    }

    #[rstest]
    #[tokio::test]
    async fn test_run_prover_async(
        parsed_prover_test_case: ParsedProverTestCase,
        #[from(prover_in_path)] _path: (),
    ) {
        let proof = run_prover_async(
            &parsed_prover_test_case.public_input,
            &parsed_prover_test_case.memory,
            &parsed_prover_test_case.trace,
            &parsed_prover_test_case.prover_config,
            &parsed_prover_test_case.prover_parameters,
        )
        .await
        .unwrap();

        assert_eq!(proof.proof_hex, parsed_prover_test_case.proof.proof_hex);
    }
}
