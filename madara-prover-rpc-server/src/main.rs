use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status};

use madara_prover_common::models::{Proof, ProverConfig, ProverParameters, PublicInput};
use prover::ProverRequest;
use stone_prover::error::ProverError;
use stone_prover::prover::run_prover_async;

use crate::cairo::{extract_run_artifacts, run_in_proof_mode};
use crate::prover::prover_server::{Prover, ProverServer};
use crate::prover::{ExecutionRequest, ExecutionResponse, ProverResponse};

mod cairo;

pub mod prover {
    tonic::include_proto!("prover");
}

fn run_cairo_program_in_proof_mode(
    execution_request: &ExecutionRequest,
) -> Result<ExecutionResponse, Status> {
    let (cairo_runner, vm) = run_in_proof_mode(&execution_request.program)
        .map_err(|e| Status::internal(format!("Failed to run Cairo program: {e}")))?;
    extract_run_artifacts(cairo_runner, vm).map_err(|e| Status::internal(e.to_string()))
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
    type ExecuteStream = ReceiverStream<Result<ExecutionResponse, Status>>;

    async fn execute(
        &self,
        request: Request<ExecutionRequest>,
    ) -> Result<Response<Self::ExecuteStream>, Status> {
        let execution_request = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(1);

        tokio::spawn(async move {
            let execution_result = run_cairo_program_in_proof_mode(&execution_request);
            let _ = tx.send(execution_result).await;
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    type ProveStream = ReceiverStream<Result<ProverResponse, Status>>;

    async fn prove(
        &self,
        request: Request<ProverRequest>,
    ) -> Result<Response<Self::ProveStream>, Status> {
        let prover_request = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(1);

        tokio::spawn(async move {
            let prover_result = call_prover(&prover_request)
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
