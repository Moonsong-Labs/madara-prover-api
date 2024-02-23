use stone_prover_sdk::cairo_vm::ExecutionError;
use tonic::Status;

pub fn execution_error_to_status(execution_error: ExecutionError) -> Status {
    match execution_error {
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
