use std::path::{Path, PathBuf};

use tempfile::tempdir;

use madara_prover_common::models::{
    PrivateInput, Proof, ProverConfig, ProverParameters, PublicInput,
};
use madara_prover_common::toolkit::{read_json_from_file, write_json_to_file};

use crate::error::ProverError;

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
        bitwise: vec![],
        ec_op: vec![],
        keccak: vec![],
        poseidon: vec![],
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
    use rstest::rstest;
    use tempfile::NamedTempFile;

    use test_fixtures::{
        parsed_prover_test_case, prover_cli_test_case, prover_in_path, read_proof_file,
        ParsedProverTestCase, ProverCliTestCase,
    };

    use super::*;

    /// Check that the Stone Prover command-line wrapper works.
    #[rstest]
    fn test_run_prover_from_command_line(
        prover_cli_test_case: ProverCliTestCase,
        #[from(prover_in_path)] _path: (),
    ) {
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
