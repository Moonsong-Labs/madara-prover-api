use stone_prover_sdk::error::{ProverError, VerifierError};
use stone_prover_sdk::fri::generate_prover_parameters;
use stone_prover_sdk::models::{
    Proof, ProofAnnotations, ProverConfig, ProverParameters, ProverWorkingDirectory,
};
use stone_prover_sdk::prover::run_prover_async;
use stone_prover_sdk::verifier::run_verifier_with_annotations_async;
use tonic::Status;

use crate::evm_adapter;
use stone_prover_sdk::cairo_vm::ExecutionArtifacts;

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
    working_dir: &mut ProverWorkingDirectory,
) -> Result<ProofAnnotations, VerifierError> {
    let annotations_file = working_dir.dir.path().join("annotations_file.txt");
    let extra_annotations_file = working_dir.dir.path().join("extra_annotations_file.txt");

    working_dir.annotations_file = Some(annotations_file.clone());
    working_dir.extra_annotations_file = Some(extra_annotations_file.clone());

    run_verifier_with_annotations_async(
        working_dir.proof_file.as_path(),
        &annotations_file,
        &extra_annotations_file,
    )
    .await?;

    Ok(ProofAnnotations {
        annotation_file: annotations_file,
        extra_output_file: extra_annotations_file,
    })
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

pub fn format_verifier_error(e: VerifierError) -> Status {
    match e {
        VerifierError::CommandError(verifier_output) => Status::invalid_argument(format!(
            "Verifier run failed ({}): {}",
            verifier_output.status,
            String::from_utf8_lossy(&verifier_output.stderr),
        )),
        VerifierError::IoError(io_error) => {
            Status::internal(format!("Could not run the verifier: {}", io_error))
        }
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
pub async fn verify_and_annotate_proof(
    proof: &mut Proof,
    working_dir: &mut ProverWorkingDirectory,
) -> Result<(), Status> {
    let _ = // TODO: return type seems worthless here
        call_verifier(working_dir)
            .await
            .map_err(format_verifier_error)?;

    let proof_file_path = working_dir.proof_file.as_path();
    let annotations_file_path = working_dir
        .annotations_file
        .clone()
        .ok_or(Status::internal("Expected annotations_file_path"))?;
    let extra_annotations_file_path = working_dir
        .extra_annotations_file
        .clone()
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
