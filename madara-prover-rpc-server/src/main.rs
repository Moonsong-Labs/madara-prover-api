use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};

use prover::ProverRequest;
use stone_prover::error::ProverError;
use stone_prover::models::{Proof, ProverConfig, ProverParameters, PublicInput};
use stone_prover::prover::run_prover_async;

use crate::prover::prover_server::{Prover, ProverServer};
use crate::prover::ProverResponse;

pub mod prover {
    tonic::include_proto!("prover");
}

async fn call_prover(prover_request: &ProverRequest) -> Result<Proof, ProverError> {
    let public_input: PublicInput = serde_json::from_str(&prover_request.public_input)?;
    let prover_config: ProverConfig = serde_json::from_str(&prover_request.prover_config)?;
    let prover_parameters: ProverParameters =
        serde_json::from_str(&prover_request.prover_parameters)?;

    run_prover_async(
        &public_input,
        &prover_request.memory,
        &prover_request.trace,
        &prover_config,
        &prover_parameters,
    )
    .await
}

#[derive(Debug, Default)]
pub struct ProverService {}

#[tonic::async_trait]
impl Prover for ProverService {
    type ProveStream = ReceiverStream<Result<ProverResponse, Status>>;

    async fn prove(
        &self,
        request: Request<ProverRequest>,
    ) -> Result<Response<Self::ProveStream>, Status> {
        let r = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(1);

        tokio::spawn(async move {
            let prover_result = call_prover(&r)
                .await
                .map(|proof| ProverResponse {
                    proof_hex: proof.proof_hex,
                })
                .map_err(|e| Status::invalid_argument(format!("Prover run failed: {e}")));
            let _ = tx.send(prover_result).await;
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = "[::1]:8080".parse().unwrap();
    let prover_service = ProverService::default();

    Server::builder()
        .add_service(ProverServer::new(prover_service))
        .serve(address)
        .await?;
    Ok(())
}
