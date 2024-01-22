use crate::cairo::ExecutionArtifacts;
use madara_prover_common::models::{Proof, ProverConfig, ProverParameters};
use stone_prover::error::ProverError;
use stone_prover::fri::generate_prover_parameters;
use stone_prover::prover::run_prover_async;
use tonic::Status;

pub async fn call_prover(
    execution_artifacts: &ExecutionArtifacts,
    prover_config: &ProverConfig,
    prover_parameters: &ProverParameters,
) -> Result<Proof, ProverError> {
    run_prover_async(
        &execution_artifacts.public_input,
        &execution_artifacts.private_input,
        &execution_artifacts.memory,
        &execution_artifacts.trace,
        prover_config,
        prover_parameters,
    )
    .await
}

pub fn format_prover_error(e: ProverError) -> Status {
    match e {
        ProverError::CommandError(prover_output) => Status::invalid_argument(format!(
            "Prover run failed ({}): {}",
            prover_output.status,
            String::from_utf8_lossy(&prover_output.stderr),
        )),
        ProverError::IoError(io_error) => {
            Status::internal(format!("Could not run the prover: {}", io_error))
        }
        ProverError::SerdeError(serde_error) => Status::invalid_argument(format!(
            "Could not parse one or more arguments: {}",
            serde_error
        )),
    }
}

pub fn get_prover_parameters(
    user_provided_parameters: Option<String>,
    nb_steps: u32,
) -> Result<ProverParameters, Status> {
    if let Some(params_str) = user_provided_parameters {
        return serde_json::from_str(&params_str)
            .map_err(|_| Status::invalid_argument("Could not read prover parameters"));
    }

    let last_layer_degree_bound = 64;
    Ok(generate_prover_parameters(
        nb_steps,
        last_layer_degree_bound,
    ))
}
