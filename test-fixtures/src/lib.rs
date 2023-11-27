use rstest::fixture;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

use test_cases::get_test_case_file_path;

use madara_prover_common::models::{
    PrivateInput, Proof, ProverConfig, ProverParameters, PublicInput,
};
use madara_prover_common::toolkit::read_json_from_file;

#[fixture]
pub fn prover_in_path() {
    // Add build dir to path for the duration of the test
    let path = std::env::var("PATH").unwrap_or_default();
    let build_dir = Path::new(env!("OUT_DIR"));
    // This will find the root of the target directory where the prover binaries
    // are put after compilation.
    let target_dir = build_dir.join("../../..").canonicalize().unwrap();

    std::env::set_var("PATH", format!("{}:{path}", target_dir.to_string_lossy()));
}

/// Reads and deserializes a JSON proof file.
pub fn read_proof_file<P: AsRef<Path>>(proof_file: P) -> Proof {
    let proof: Proof = read_json_from_file(proof_file).expect("Could not open proof file");
    proof
}

/// All the files forming a complete prover test case.
pub struct ProverTestCase {
    pub public_input_file: PathBuf,
    pub prover_config_file: PathBuf,
    pub prover_parameter_file: PathBuf,
    pub memory_file: PathBuf,
    pub trace_file: PathBuf,
    pub proof_file: PathBuf,
}

#[fixture]
pub fn fibonacci() -> ProverTestCase {
    let public_input_file = get_test_case_file_path("fibonacci/fibonacci_public_input.json");
    let prover_config_file = get_test_case_file_path("fibonacci/cpu_air_prover_config.json");
    let prover_parameter_file = get_test_case_file_path("fibonacci/cpu_air_params.json");
    let memory_file = get_test_case_file_path("fibonacci/fibonacci_memory.bin");
    let trace_file = get_test_case_file_path("fibonacci/fibonacci_trace.bin");
    let proof_file = get_test_case_file_path("fibonacci/fibonacci_proof.json");

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
pub struct ProverCliTestCase {
    pub public_input_file: PathBuf,
    pub private_input_file: NamedTempFile,
    pub prover_config_file: PathBuf,
    pub prover_parameter_file: PathBuf,
    pub proof: Proof,
}

#[fixture]
pub fn prover_cli_test_case(#[from(fibonacci)] files: ProverTestCase) -> ProverCliTestCase {
    // Generate the private input in a temporary file
    let private_input_file =
        NamedTempFile::new().expect("Creating temporary private input file failed");
    let private_input = PrivateInput {
        memory_path: files.memory_file.clone(),
        trace_path: files.trace_file.clone(),
        pedersen: vec![],
        range_check: vec![],
        ecdsa: vec![],
        bitwise: vec![],
        ec_op: vec![],
        keccak: vec![],
        poseidon: vec![],
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

pub struct ParsedProverTestCase {
    pub public_input: PublicInput,
    pub memory: Vec<u8>,
    pub trace: Vec<u8>,
    pub prover_config: ProverConfig,
    pub prover_parameters: ProverParameters,
    pub proof: Proof,
}

#[fixture]
pub fn parsed_prover_test_case(#[from(fibonacci)] files: ProverTestCase) -> ParsedProverTestCase {
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
