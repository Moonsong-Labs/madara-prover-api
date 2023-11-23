use tonic::codegen::tokio_stream::StreamExt;
use tonic::{Status, Streaming};

use crate::prover::prover_client::ProverClient;
use crate::prover::{ExecutionRequest, ExecutionResponse, ProverRequest, ProverResponse};
use madara_prover_common::models::{Proof, ProverConfig, ProverParameters, PublicInput};

async fn wait_for_streamed_response<ResponseType>(
    stream: Streaming<ResponseType>,
) -> Result<ResponseType, Status> {
    if let Some(response) = stream.take(1).next().await {
        return response;
    }

    Err(Status::cancelled("server-side stream was dropped"))
}

pub async fn execute_program(
    client: &mut ProverClient<tonic::transport::Channel>,
    program_content: Vec<u8>,
) -> Result<ExecutionResponse, Status> {
    let request = tonic::Request::new(ExecutionRequest {
        program: program_content,
    });
    let execution_stream = client.execute(request).await?.into_inner();
    wait_for_streamed_response(execution_stream).await
}

fn unpack_prover_response(prover_result: Result<ProverResponse, Status>) -> Result<Proof, Status> {
    match prover_result {
        Ok(prover_response) => serde_json::from_str(&prover_response.proof)
            .map_err(|e| Status::internal(format!("Could not read prover output: {}", e))),
        Err(status) => Err(status),
    }
}

pub async fn prove_execution(
    client: &mut ProverClient<tonic::transport::Channel>,
    public_input: PublicInput,
    memory: Vec<u8>,
    trace: Vec<u8>,
    prover_config: ProverConfig,
    prover_parameters: ProverParameters,
) -> Result<Proof, Status> {
    let public_input_str = serde_json::to_string(&public_input).unwrap();
    let prover_config_str = serde_json::to_string(&prover_config).unwrap();
    let prover_parameters_str = serde_json::to_string(&prover_parameters).unwrap();

    let request = tonic::Request::new(ProverRequest {
        public_input: public_input_str,
        memory,
        trace,
        prover_config: prover_config_str,
        prover_parameters: prover_parameters_str,
    });
    let prover_stream = client.prove(request).await?.into_inner();
    let prover_result = wait_for_streamed_response(prover_stream).await;
    unpack_prover_response(prover_result)
}
