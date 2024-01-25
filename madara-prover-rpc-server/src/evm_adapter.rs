use std::path::Path;

use madara_prover_common::toolkit::read_json_from_file;
use stark_evm_adapter::{
    annotation_parser::{split_fri_merkle_statements, SplitProofs},
    annotated_proof::AnnotatedProof,
};
use std::io::BufRead;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SplitProverError {
    #[error("I/O Error")]
    Io(#[from] std::io::Error),
    #[error("Error involving split proof")]
    ProofParseError(#[from] stark_evm_adapter::errors::ParseError)
}

/// Uses stark-evm-adapter to split the proof. 
pub fn split_proof(
    proof_file: &Path,
    annotations_file: &Path,
    extra_annotations_file: &Path
) -> Result<SplitProofs, SplitProverError> {
    // 'proof_file' is not expected to have an annotations or an extra_annotations field.
    // but this will cause an error if we try to parse it as an AnnotatedProof without these
    // fields.
    // 
    // since these values are given as separate files, we will with the proof as a JSON object
    // and add the 'annotations' and 'extra_annotations' fields manually, as the `stark-evm-adapter`
    // binary does.
    let mut proof_json: serde_json::Value = read_json_from_file(proof_file)?;
    proof_json["annotations"] = load_annotations_file(annotations_file)?.into();
    proof_json["extra_annotations"] = load_annotations_file(extra_annotations_file)?.into();

    let annotated_proof: AnnotatedProof = serde_json::from_value(proof_json)
        .unwrap(); // TODO

    let split_proofs: SplitProofs = split_fri_merkle_statements(annotated_proof)?;
    
    Ok(split_proofs)
}

/// Reads an annotations file, parsing it into a vec of strings suitable for stark-evm-adapter's
/// AnnotatedProof struct.
/// May be called for both "annotations" and "extra-annotations".
pub fn load_annotations_file(file: &Path) -> std::io::Result<Vec<String>> {
    let file = std::fs::File::open(file)?;
    let lines: Vec<String> = std::io::BufReader::new(file)
        .lines()
        .map(|line| line.unwrap())
        .collect();
    Ok(lines)
}

mod tests {
    #[test]
    fn split_proof_works_with_empty_bootloader_proof() {
        let annotated_proof_file = test_cases::get_test_case_file_path("bootloader/empty_bootloader_proof/annotated_proof.json");
        let annotations_file = test_cases::get_test_case_file_path("bootloader/empty_bootloader_proof/annotations.txt");
        let extra_annotations_file = test_cases::get_test_case_file_path("bootloader/empty_bootloader_proof/extra_annotations.txt");
        let split_proofs = split_proof(
            &annotated_proof_file,
            &annotations_file,
            &extra_annotations_file
        ).unwrap();

        assert!(split_proofs.merkle_statements.len() > 0);
        assert!(split_proofs.fri_merkle_statements.len() > 0);
        assert!(split_proofs.main_proof.proof.len() > 0);
    }
}