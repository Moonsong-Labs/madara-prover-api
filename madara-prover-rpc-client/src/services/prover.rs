use cairo_vm::air_private_input::AirPrivateInput;
use tonic::Status;

use madara_prover_common::models::{Proof, ProverConfig, ProverParameters, PublicInput};

use prover_proto::prover_client::ProverClient;
use prover_proto::{ExecutionRequest, ExecutionResponse, ProverRequest, ProverResponse};

pub mod prover_proto {
    tonic::include_proto!("prover");
}

/// Execute a program in proof mode and retrieve the execution artifacts.
pub async fn execute_program(
    client: &mut ProverClient<tonic::transport::Channel>,
    program_content: Vec<u8>,
) -> Result<ExecutionResponse, Status> {
    let request = tonic::Request::new(ExecutionRequest {
        program: program_content,
        prover_config: None,
        prover_parameters: None,
    });
    client
        .execute(request)
        .await
        .map(|response| response.into_inner())
}

fn unpack_prover_response(prover_result: Result<ProverResponse, Status>) -> Result<Proof, Status> {
    match prover_result {
        Ok(prover_response) => serde_json::from_str(&prover_response.proof)
            .map_err(|e| Status::internal(format!("Could not read prover output: {}", e))),
        Err(status) => Err(status),
    }
}

/// Prove the execution of a program.
pub async fn prove_execution(
    client: &mut ProverClient<tonic::transport::Channel>,
    public_input: PublicInput,
    private_input: AirPrivateInput,
    memory: Vec<u8>,
    trace: Vec<u8>,
    prover_config: ProverConfig,
    prover_parameters: ProverParameters,
) -> Result<Proof, Status> {
    let public_input_str = serde_json::to_string(&public_input).unwrap();
    let private_input_str =
        serde_json::to_string(&private_input.to_serializable("".to_string(), "".to_string()))
            .unwrap();
    let prover_config_str = serde_json::to_string(&prover_config).unwrap();
    let prover_parameters_str = serde_json::to_string(&prover_parameters).unwrap();

    let request = tonic::Request::new(ProverRequest {
        public_input: public_input_str,
        private_input: private_input_str,
        memory,
        trace,
        prover_config: prover_config_str,
        prover_parameters: prover_parameters_str,
    });
    let prover_response = client.prove(request).await;
    let prover_result = prover_response.map(|response| response.into_inner());
    unpack_prover_response(prover_result)
}

/// Execute and prove a program.
pub async fn execute_and_prove(
    client: &mut ProverClient<tonic::transport::Channel>,
    program_content: Vec<u8>,
    prover_config: Option<ProverConfig>,
    prover_parameters: Option<ProverParameters>,
) -> Result<Proof, Status> {
    let serialized_prover_config =
        prover_config.map(|config| serde_json::to_string(&config).unwrap());
    let serialized_prover_parameters =
        prover_parameters.map(|params| serde_json::to_string(&params).unwrap());

    let request = ExecutionRequest {
        program: program_content,
        prover_config: serialized_prover_config,
        prover_parameters: serialized_prover_parameters,
    };

    let prover_result = client
        .execute_and_prove(request)
        .await
        .map(|response| response.into_inner());
    unpack_prover_response(prover_result)
}
