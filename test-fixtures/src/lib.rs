use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use cairo_vm::air_private_input::{AirPrivateInput, AirPrivateInputSerializable};
use cairo_vm::vm::runners::builtin_runner::OUTPUT_BUILTIN_NAME;
use cairo_vm::Felt252;
use rstest::fixture;
use tempfile::NamedTempFile;

use stone_prover_sdk::json::read_json_from_file;
use stone_prover_sdk::models::{Proof, ProverConfig, ProverParameters, PublicInput};
use test_cases::get_test_case_file_path;

/// Reads and deserializes a JSON proof file.
pub fn read_proof_file<P: AsRef<Path>>(proof_file: P) -> Proof {
    let proof: Proof = read_json_from_file(proof_file).expect("Could not open proof file");
    proof
}

/// All the files forming a complete prover test case.
pub struct ProverTestCase {
    pub program_file: PathBuf,
    pub compiled_program_file: PathBuf,
    pub public_input_file: PathBuf,
    pub private_input_file: PathBuf,
    pub prover_config_file: PathBuf,
    pub prover_parameter_file: PathBuf,
    pub memory_file: PathBuf,
    pub trace_file: PathBuf,
    pub proof_file: PathBuf,
}

#[fixture]
pub fn fibonacci() -> ProverTestCase {
    let program_file = get_test_case_file_path("fibonacci/fibonacci.cairo");
    let compiled_program_file = get_test_case_file_path("fibonacci/fibonacci_compiled.json");
    let public_input_file = get_test_case_file_path("fibonacci/fibonacci_public_input.json");
    let private_input_file = get_test_case_file_path("fibonacci/fibonacci_private_input.json");
    let prover_config_file = get_test_case_file_path("fibonacci/cpu_air_prover_config.json");
    let prover_parameter_file = get_test_case_file_path("fibonacci/cpu_air_params.json");
    let memory_file = get_test_case_file_path("fibonacci/fibonacci_memory.bin");
    let trace_file = get_test_case_file_path("fibonacci/fibonacci_trace.bin");
    let proof_file = get_test_case_file_path("fibonacci/fibonacci_proof.json");

    ProverTestCase {
        program_file,
        compiled_program_file,
        public_input_file,
        private_input_file,
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
    let private_input = AirPrivateInput(HashMap::new()).to_serializable(
        files.trace_file.to_string_lossy().into_owned(),
        files.memory_file.to_string_lossy().into_owned(),
    );

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
    pub compiled_program: Vec<u8>,
    pub public_input: PublicInput,
    pub private_input: AirPrivateInput,
    pub memory: Vec<u8>,
    pub trace: Vec<u8>,
    pub prover_config: ProverConfig,
    pub prover_parameters: ProverParameters,
    pub proof: Proof,
}

#[fixture]
pub fn parsed_prover_test_case(#[from(fibonacci)] files: ProverTestCase) -> ParsedProverTestCase {
    let compiled_program = std::fs::read(files.compiled_program_file).unwrap();
    let public_input: PublicInput = read_json_from_file(files.public_input_file).unwrap();
    let private_input: AirPrivateInputSerializable =
        read_json_from_file(files.private_input_file).unwrap();
    let prover_config: ProverConfig = read_json_from_file(files.prover_config_file).unwrap();
    let prover_parameters: ProverParameters =
        read_json_from_file(files.prover_parameter_file).unwrap();
    let memory = std::fs::read(files.memory_file).unwrap();
    let trace = std::fs::read(files.trace_file).unwrap();

    let proof = read_proof_file(&files.proof_file);

    ParsedProverTestCase {
        compiled_program,
        public_input,
        private_input: private_input.into(),
        memory,
        trace,
        prover_config,
        prover_parameters,
        proof,
    }
}

/// Reads a memory file as (address, value) pairs.
pub fn read_memory_pairs<R: Read>(
    mut reader: R,
    addr_size: usize,
    felt_size: usize,
) -> Vec<(u64, Felt252)> {
    let pair_size = addr_size + felt_size;
    let mut memory = Vec::<(u64, Felt252)>::new();

    loop {
        let mut element = Vec::with_capacity(pair_size);
        let n = reader
            .by_ref()
            .take(pair_size as u64)
            .read_to_end(&mut element)
            .unwrap();
        if n == 0 {
            break;
        }
        assert_eq!(n, pair_size);

        let (address_bytes, value_bytes) = element.split_at(addr_size);
        let address = {
            let mut value = 0;
            for (index, byte) in address_bytes[..8].iter().enumerate() {
                value += u64::from(*byte) << (index * 8);
            }
            value
        };
        let value = Felt252::from_bytes_le_slice(value_bytes);
        memory.push((address, value));
    }

    memory
}

/// Converts a vector of (address, value) pairs to a hashmap. Panics if a key appears more than once.
fn memory_pairs_to_hashmap(pairs: Vec<(u64, Felt252)>) -> HashMap<u64, Felt252> {
    let mut map = HashMap::new();

    for (address, value) in pairs.into_iter() {
        assert!(!map.contains_key(&address));
        map.insert(address, value);
    }

    map
}

/// Checks that the two specified memory files describe the same memory, regardless of the Python vs Rust VM formats.
pub fn assert_memory_eq(actual: &Vec<u8>, expected: &Vec<u8>) {
    assert_eq!(actual.len() % 40, 0);
    assert_eq!(expected.len() % 40, 0);

    let actual_memory_pairs = read_memory_pairs(actual.as_slice(), 8, 32);
    let expected_memory_pairs = read_memory_pairs(expected.as_slice(), 8, 32);

    let actual_memory = memory_pairs_to_hashmap(actual_memory_pairs);
    let expected_memory = memory_pairs_to_hashmap(expected_memory_pairs);

    assert_eq!(actual_memory, expected_memory);
}

pub fn assert_private_input_eq(actual: AirPrivateInput, expected: AirPrivateInput) {
    let actual_map = {
        let mut map = actual.0;
        map.remove(OUTPUT_BUILTIN_NAME);
        map
    };

    assert_eq!(actual_map, expected.0);
}
