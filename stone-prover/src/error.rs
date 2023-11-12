use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProverError {
    #[error("prover could not be launched")]
    IoError(#[from] std::io::Error),
    #[error("prover run failed")]
    CommandError(std::process::Output),
    #[error("the format of a JSON file is invalid")]
    SerdeError(#[from] serde_json::Error),
}
