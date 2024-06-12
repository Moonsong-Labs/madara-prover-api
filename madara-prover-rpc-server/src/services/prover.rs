use cairo_vm::air_private_input::{AirPrivateInput, AirPrivateInputSerializable};
use tonic::{Request, Response, Status};

use crate::cairo::execution_error_to_status;
use crate::services::common;
use crate::services::common::format_prover_error;
use crate::services::prover::prover_proto::prover_server::Prover;
use crate::services::prover::prover_proto::{
    ExecutionRequest, ExecutionResponse, ProverRequest, ProverResponse,
};
use stone_prover_sdk::cairo_vm::{
    extract_execution_artifacts, run_in_proof_mode, ExecutionArtifacts, ExecutionError,
};
use stone_prover_sdk::error::ProverError;
use stone_prover_sdk::models::{Layout, Proof, ProverConfig, ProverWorkingDirectory};

pub mod prover_proto {
    tonic::include_proto!("prover");
}

fn run_cairo_program_in_proof_mode(
    program: &[u8],
    layout: Layout,
) -> Result<ExecutionArtifacts, ExecutionError> {
    let allow_missing_builtins = Some(false);
    let cairo_runner = run_in_proof_mode(program, layout, allow_missing_builtins)?;
    extract_execution_artifacts(cairo_runner)
}

fn format_execution_result(
    execution_result: Result<ExecutionArtifacts, ExecutionError>,
) -> Result<ExecutionResponse, Status> {
    match execution_result {
        Ok(artifacts) => serde_json::to_string(&artifacts.public_input)
            .map(|public_input_str| ExecutionResponse {
                public_input: public_input_str,
                memory: artifacts.memory,
                trace: artifacts.trace,
            })
            .map_err(|_| Status::internal("Failed to serialize public input")),
        Err(e) => Err(execution_error_to_status(e)),
    }
}

/// Formats the output of the prover subprocess into the server response.
fn format_prover_result(
    prover_result: Result<(Proof, ProverWorkingDirectory), ProverError>,
) -> Result<ProverResponse, Status> {
    match prover_result {
        Ok((proof, _)) => serde_json::to_string(&proof)
            .map(|proof_str| ProverResponse { proof: proof_str })
            .map_err(|_| Status::internal("Could not parse the proof returned by the prover")),
        Err(e) => Err(format_prover_error(e)),
    }
}

fn get_prover_config(user_provided_config: Option<String>) -> Result<ProverConfig, Status> {
    if let Some(config_str) = user_provided_config {
        return serde_json::from_str(&config_str)
            .map_err(|_| Status::invalid_argument("Could not read prover config"));
    }

    Ok(ProverConfig::default())
}

#[derive(Debug, Default)]
pub struct ProverService {}

#[tonic::async_trait]
impl Prover for ProverService {
    async fn execute(
        &self,
        request: Request<ExecutionRequest>,
    ) -> Result<Response<ExecutionResponse>, Status> {
        let execution_request = request.into_inner();

        let layout = Layout::StarknetWithKeccak;
        let execution_result = run_cairo_program_in_proof_mode(&execution_request.program, layout);
        let execution_result = format_execution_result(execution_result);

        execution_result.map(Response::new)
    }

    async fn prove(
        &self,
        request: Request<ProverRequest>,
    ) -> Result<Response<ProverResponse>, Status> {
        let ProverRequest {
            public_input: public_input_str,
            private_input: private_input_str,
            memory,
            trace,
            prover_config: prover_config_str,
            prover_parameters: prover_parameters_str,
        } = request.into_inner();

        let public_input = serde_json::from_str(&public_input_str)
            .map_err(|_| Status::invalid_argument("Could not deserialize public input"))?;
        let private_input: AirPrivateInputSerializable =
            serde_json::from_str(&private_input_str)
                .map_err(|_| Status::invalid_argument("Could not deserialize private input"))?;
        let prover_config = serde_json::from_str(&prover_config_str)
            .map_err(|_| Status::invalid_argument("Could not deserialize prover config"))?;
        let prover_parameters = serde_json::from_str(&prover_parameters_str)
            .map_err(|_| Status::invalid_argument("Could not deserialize prover parameters"))?;

        let execution_artifacts = ExecutionArtifacts {
            public_input,
            private_input: AirPrivateInput::from(private_input),
            memory,
            trace,
        };

        let prover_result =
            common::call_prover(&execution_artifacts, &prover_config, &prover_parameters).await;
        let formatted_result = format_prover_result(prover_result);

        formatted_result.map(Response::new)
    }

    async fn execute_and_prove(
        &self,
        request: Request<ExecutionRequest>,
    ) -> Result<Response<ProverResponse>, Status> {
        let ExecutionRequest {
            program,
            prover_config: prover_config_str,
            prover_parameters: prover_parameters_str,
        } = request.into_inner();

        let prover_config = get_prover_config(prover_config_str)?;
        let layout = Layout::StarknetWithKeccak;

        let execution_artifacts = run_cairo_program_in_proof_mode(&program, layout);
        let execution_artifacts = execution_artifacts
            .map_err(|e| Status::internal(format!("Failed to run program: {e}")))?;

        let prover_parameters = common::get_prover_parameters(
            prover_parameters_str,
            execution_artifacts.public_input.n_steps,
        )?;

        let prover_result =
            common::call_prover(&execution_artifacts, &prover_config, &prover_parameters).await;

        format_prover_result(prover_result).map(Response::new)
    }
}
