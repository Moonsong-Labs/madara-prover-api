use tonic::Status;

use madara_prover_common::models::Proof;
use starknet_prover_proto::{StarknetExecutionRequest, StarknetProverResponse};

use crate::services::starknet_prover::starknet_prover_proto::starknet_prover_client::StarknetProverClient;

pub mod starknet_prover_proto {
    tonic::include_proto!("starknet_prover");
}

fn unpack_prover_response(
    prover_result: Result<StarknetProverResponse, Status>,
) -> Result<Proof, Status> {
    match prover_result {
        Ok(prover_response) => serde_json::from_str(&prover_response.proof)
            .map_err(|e| Status::internal(format!("Could not read prover output: {}", e))),
        Err(status) => Err(status),
    }
}

/// Execute programs/PIEs with the Starknet bootloader and generate a proof.
pub async fn execute_and_prove(
    client: &mut StarknetProverClient<tonic::transport::Channel>,
    programs: Vec<Vec<u8>>,
    pies: Vec<Vec<u8>>,
    split_proof: bool,
) -> Result<Proof, Status> {
    let request = StarknetExecutionRequest {
        programs,
        pies,
        split_proof,
    };

    let prover_result = client
        .execute_and_prove(request)
        .await
        .map(|response| response.into_inner());
    unpack_prover_response(prover_result)
}
