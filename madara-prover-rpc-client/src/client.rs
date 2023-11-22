use tonic::codegen::tokio_stream::StreamExt;
use tonic::{Status, Streaming};

use crate::prover::prover_client::ProverClient;
use crate::prover::{ExecutionRequest, ExecutionResponse, ProverRequest, ProverResponse};

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

pub async fn call_prover(
    client: &mut ProverClient<tonic::transport::Channel>,
    public_input: String,
    memory: Vec<u8>,
    trace: Vec<u8>,
    prover_config: String,
    prover_parameters: String,
) -> Result<ProverResponse, Status> {
    let request = tonic::Request::new(ProverRequest {
        public_input,
        memory,
        trace,
        prover_config,
        prover_parameters,
    });
    let prover_stream = client.prove(request).await?.into_inner();
    wait_for_streamed_response(prover_stream).await
}
