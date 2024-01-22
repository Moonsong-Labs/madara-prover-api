use std::path::Path;
use std::borrow::BorrowMut;

use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

use madara_prover_common::models::{Proof, ProverConfig, ProverParameters, ProofAnnotations};
use prover::ProverRequest;
use stone_prover::error::ProverError;
use stone_prover::fri::generate_prover_parameters;
use stone_prover::prover::{run_prover_async, run_verifier_async};

use crate::cairo::{
    extract_execution_artifacts, run_in_proof_mode, ExecutionArtifacts, ExecutionError,
};
use crate::error::ServerError;
use crate::services::prover::prover_proto::prover_server::ProverServer;
use crate::services::prover::ProverService;
use crate::services::starknet_prover::starknet_prover_proto::starknet_prover_server::StarknetProverServer;
use crate::services::starknet_prover::StarknetProverService;

pub mod cairo;
pub mod evm_adapter;
pub mod error;
pub mod services;

pub mod prover {
    tonic::include_proto!("prover");
}

fn run_cairo_program_in_proof_mode(program: &[u8]) -> Result<ExecutionArtifacts, ExecutionError> {
    let (cairo_runner, vm) = run_in_proof_mode(program)?;
    extract_execution_artifacts(cairo_runner, vm)
}

async fn call_prover(
    execution_artifacts: &ExecutionArtifacts,
    prover_config: &ProverConfig,
    prover_parameters: &ProverParameters,
) -> Result<Proof, ProverError> {
    run_prover_async(
        &execution_artifacts.public_input,
        &execution_artifacts.memory,
        &execution_artifacts.trace,
        prover_config,
        prover_parameters,
    )
    .await
}

async fn call_verifier(
    proof: &mut Proof,
) -> Result<ProofAnnotations, ProverError> {

    assert!(proof.working_dir.is_some(),
        "Cannot call verifier without working dir.");

    let mut working_dir = proof.working_dir.as_mut().unwrap();
    let proof_file = working_dir
        .proof_file
        .as_path();

    assert!(working_dir.annotations_file.is_none(),
        "Annotations file should not already exist");
    assert!(working_dir.extra_annotations_file.is_none(),
        "Extra annotations file should not already exist");

    let annotations_file = working_dir.dir.path().join("annotations_file.txt");
    let extra_annotations_file = working_dir.dir.path().join("extra_annotations_file.txt");

    working_dir.annotations_file = Some(annotations_file.clone());
    working_dir.extra_annotations_file = Some(extra_annotations_file.clone());

    run_verifier_async(
        working_dir.proof_file.as_path(),
        &annotations_file,
        &extra_annotations_file,
    )
    .await
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
        Err(e) => Err(e.into()),
    }
}

fn format_prover_error(e: ProverError) -> Status {
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
        ProverError::InternalError => Status::internal("An internal error occurred"),
    }
}

/// Formats the output of the prover subprocess into the server response.
fn format_prover_result(
    prover_result: Result<Proof, ProverError>,
) -> Result<ProverResponse, Status> {
    match prover_result {
        Ok(proof) => serde_json::to_string(&proof)
            .map(|proof_str| ProverResponse { proof: proof_str, split_proof: false })
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

fn get_prover_parameters(
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

async fn verify_and_annotate_proof(proof: &mut Proof) -> Result<(), ProverError> {
    let verifier_result =
        call_verifier(proof).await;
    
    let working_dir = proof.working_dir.as_ref().unwrap(); // TODO:
    let proof_file_path = working_dir.proof_file.as_path();
    let annotations_file_path = working_dir.annotations_file.clone().ok_or(ProverError::InternalError)?;
    let extra_annotations_file_path = working_dir.extra_annotations_file.clone().ok_or(ProverError::InternalError)?;

    let split_proof =  {
        let split_proof = evm_adapter::split_proof(
            proof_file_path,
            annotations_file_path.as_path(),
            extra_annotations_file_path.as_path(),
        ).unwrap();
        proof.working_dir = None;
        Some(split_proof)
    };

    Ok(())
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

        let execution_result = run_cairo_program_in_proof_mode(&execution_request.program);
        let execution_result = format_execution_result(execution_result);

        execution_result.map(Response::new)
    }

    async fn prove(
        &self,
        request: Request<ProverRequest>,
    ) -> Result<Response<ProverResponse>, Status> {
        let ProverRequest {
            public_input: public_input_str,
            memory,
            trace,
            prover_config: prover_config_str,
            prover_parameters: prover_parameters_str,
            split_proof: build_split_proof,
        } = request.into_inner();

        let public_input = serde_json::from_str(&public_input_str)
            .map_err(|_| Status::invalid_argument("Could not deserialize public input"))?;
        let prover_config = serde_json::from_str(&prover_config_str)
            .map_err(|_| Status::invalid_argument("Could not deserialize prover config"))?;
        let prover_parameters = serde_json::from_str(&prover_parameters_str)
            .map_err(|_| Status::invalid_argument("Could not deserialize prover parameters"))?;

        let execution_artifacts = ExecutionArtifacts {
            public_input,
            memory,
            trace,
        };

        let mut prover_result =
            call_prover(&execution_artifacts, &prover_config, &prover_parameters).await;

        // If split proof was requested, build it
        if build_split_proof == Some(true) && prover_result.is_ok() {
            let res = verify_and_annotate_proof(prover_result.as_mut().expect("Result checked above")).await;
            if res.is_err() {
                prover_result = Err(res.err().unwrap());
            }
        };

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

        let execution_artifacts = run_cairo_program_in_proof_mode(&program);
        let execution_artifacts = execution_artifacts
            .map_err(|e| Status::internal(format!("Failed to run program: {e}")))?;

        let prover_parameters = get_prover_parameters(
            prover_parameters_str,
            execution_artifacts.public_input.n_steps,
        )?;

        let prover_result =
            call_prover(&execution_artifacts, &prover_config, &prover_parameters).await;

        format_prover_result(prover_result).map(Response::new)
    }
}
>>>>>>> ce287ce (Basic verifier invocation works)

pub enum BindAddress<'a> {
    Tcp(std::net::SocketAddr),
    UnixSocket(&'a Path),
}

pub async fn run_grpc_server(bind_address: BindAddress<'_>) -> Result<(), ServerError> {
    let prover_service = ProverService::default();
    let starknet_prover_service = StarknetProverService::default();

    let builder = Server::builder()
        .add_service(ProverServer::new(prover_service))
        .add_service(StarknetProverServer::new(starknet_prover_service));

    match bind_address {
        BindAddress::Tcp(address) => builder.serve(address).await?,
        BindAddress::UnixSocket(socket_path) => {
            let uds = UnixListener::bind(socket_path)?;
            let uds_stream = UnixListenerStream::new(uds);
            builder.serve_with_incoming(uds_stream).await?
        }
    }

    Ok(())
}
