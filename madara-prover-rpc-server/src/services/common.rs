use crate::cairo::ExecutionArtifacts;
use crate::evm_adapter;
use madara_prover_common::models::{Proof, ProofAnnotations, ProverConfig, ProverParameters, ProverWorkingDirectory};
use stone_prover::error::ProverError;
use stone_prover::fri::generate_prover_parameters;
use stone_prover::prover::{run_prover_async, run_verifier_async};
use tonic::Status;

pub async fn call_prover(
    execution_artifacts: &ExecutionArtifacts,
    prover_config: &ProverConfig,
    prover_parameters: &ProverParameters,
) -> Result<(Proof, ProverWorkingDirectory), ProverError> {
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

pub async fn call_verifier(
    proof: &mut Proof,
    working_dir: &mut ProverWorkingDirectory,
) -> Result<ProofAnnotations, ProverError> {

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

/// Calls `cpu_air_verifier` to verify the proof and produce annotations, then uses
/// `stark-evm-adapter` to split the proof. The given Proof will then be modified to contain
/// this additional split-proof.
pub async fn verify_and_annotate_proof(proof: &mut Proof, working_dir: &mut ProverWorkingDirectory) -> Result<(), Status> {
    let _ = // TODO: return type seems worthless here
        call_verifier(proof, working_dir)
        .await
        .map_err(|e| format_prover_error(e))?;

    let proof_file_path = working_dir.proof_file.as_path();
    let annotations_file_path = working_dir.annotations_file.clone()
        .ok_or(Status::internal("Expected annotations_file_path"))?;
    let extra_annotations_file_path = working_dir.extra_annotations_file.clone()
        .ok_or(Status::internal("Expected extra_annotations_file_path"))?;

    let split_proof = evm_adapter::split_proof(
        proof_file_path,
        annotations_file_path.as_path(),
        extra_annotations_file_path.as_path(),
    )
    .map_err(|_| Status::internal("Unable to generate split proof"))?;
    
    proof.split_proofs = Some(split_proof);

    Ok(())
}