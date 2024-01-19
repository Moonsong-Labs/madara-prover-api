use std::path::Path;

use madara_prover_common::toolkit::read_json_from_file;
use stark_evm_adapter::{
    annotation_parser::{split_fri_merkle_statements, SplitProofs},
    annotated_proof::AnnotatedProof,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SplitProverError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("Error involving split proof")]
    ProofParseError(#[from] stark_evm_adapter::errors::ParseError)
}

/// Uses stark-evm-adapter to split the proof. The given proof JSON file must contain an
/// "annotations" field, probably by running stone prover with --generate_annotations
fn split_proof(annotated_proof_file: &Path) -> Result<SplitProofs, SplitProverError> {
    let annotated_proof: AnnotatedProof = read_json_from_file(annotated_proof_file)?;
    let split_proofs: SplitProofs = split_fri_merkle_statements(annotated_proof.clone())?;
    
    Ok(split_proofs)
}

mod tests {
    use super::*;
    
    #[test]
    fn split_proof_works() {
        let annotated_proof_file = test_cases::get_test_case_file_path("annotated_proof.json");
        let split_proofs = split_proof(&annotated_proof_file).unwrap();

        assert!(split_proofs.merkle_statements.len() > 0);
        assert!(split_proofs.fri_merkle_statements.len() > 0);
        assert!(split_proofs.main_proof.proof.len() > 0);
    }
}