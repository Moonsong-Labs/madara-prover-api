use tonic::codegen::tokio_stream::StreamExt;
use tonic::Status;

use crate::prover::prover_client::ProverClient;
use crate::prover::{ProverRequest, ProverResponse};

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
    if let Some(prover_result) = prover_stream.take(1).next().await {
        return prover_result;
    }

    Err(Status::cancelled("Server-side stream was dropped"))
}
