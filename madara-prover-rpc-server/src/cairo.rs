use bincode::error::EncodeError;
use cairo_vm::air_public_input::PublicInputError;
use cairo_vm::cairo_run::{
    cairo_run, write_encoded_memory, write_encoded_trace, CairoRunConfig, EncodeTraceError,
};
use cairo_vm::hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor;
use cairo_vm::vm::errors::cairo_run_errors::CairoRunError;
use cairo_vm::vm::errors::trace_errors::TraceError;
use cairo_vm::vm::runners::cairo_runner::CairoRunner;
use cairo_vm::vm::vm_core::VirtualMachine;
use thiserror::Error;
use tonic::Status;

use madara_prover_common::models::PublicInput;

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error(transparent)]
    RunFailed(#[from] CairoRunError),
    #[error(transparent)]
    GeneratePublicInput(#[from] PublicInputError),
    #[error(transparent)]
    GenerateTrace(#[from] TraceError),
    #[error(transparent)]
    EncodeMemory(EncodeTraceError),
    #[error(transparent)]
    EncodeTrace(EncodeTraceError),
    #[error(transparent)]
    SerializePublicInput(#[from] serde_json::Error),
}

impl From<ExecutionError> for Status {
    fn from(value: ExecutionError) -> Self {
        match value {
            ExecutionError::RunFailed(cairo_run_error) => {
                Status::internal(format!("Failed to run Cairo program: {}", cairo_run_error))
            }
            ExecutionError::GeneratePublicInput(public_input_error) => Status::internal(format!(
                "Failed to generate public input: {}",
                public_input_error
            )),
            ExecutionError::GenerateTrace(trace_error) => Status::internal(format!(
                "Failed to generate execution trace: {}",
                trace_error
            )),
            ExecutionError::EncodeMemory(encode_error) => Status::internal(format!(
                "Failed to encode execution memory: {}",
                encode_error
            )),
            ExecutionError::EncodeTrace(encode_error) => Status::internal(format!(
                "Failed to encode execution memory: {}",
                encode_error
            )),
            ExecutionError::SerializePublicInput(serde_error) => {
                Status::internal(format!("Failed to serialize public input: {}", serde_error))
            }
        }
    }
}

/// An in-memory writer for bincode encoding.
pub struct MemWriter {
    pub buf: Vec<u8>,
}

impl MemWriter {
    pub fn new() -> Self {
        Self { buf: vec![] }
    }
}
impl bincode::enc::write::Writer for MemWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        self.buf.extend_from_slice(bytes);
        Ok(())
    }
}

/// Run a Cairo program in proof mode.
///
/// * `program_content`: Compiled program content.
pub fn run_in_proof_mode(
    program_content: &[u8],
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

    let mut hint_processor = BuiltinHintProcessor::new_empty();

    cairo_run(program_content, &cairo_run_config, &mut hint_processor)
}

pub struct ExecutionArtifacts {
    pub public_input: PublicInput,
    pub memory: Vec<u8>,
    pub trace: Vec<u8>,
}

// TODO: split in two (extract data + format to ExecutionResponse)
/// Extracts execution artifacts from the runner and VM (after execution).
///
/// * `cairo_runner` Cairo runner object.
/// * `vm`: Cairo VM object.
pub fn extract_execution_artifacts(
    cairo_runner: CairoRunner,
    vm: VirtualMachine,
) -> Result<ExecutionArtifacts, ExecutionError> {
    let cairo_vm_public_input = cairo_runner.get_air_public_input(&vm)?;

    let memory = cairo_runner.relocated_memory.clone();
    let trace = vm.get_relocated_trace()?;

    let mut memory_writer = MemWriter::new();
    write_encoded_memory(&memory, &mut memory_writer).map_err(ExecutionError::EncodeMemory)?;
    let memory_raw = memory_writer.buf;

    let mut trace_writer = MemWriter::new();
    write_encoded_trace(trace, &mut trace_writer).map_err(ExecutionError::EncodeTrace)?;
    let trace_raw = trace_writer.buf;

    let public_input = PublicInput::try_from(cairo_vm_public_input)?;

    Ok(ExecutionArtifacts {
        public_input,
        memory: memory_raw,
        trace: trace_raw,
    })
}
