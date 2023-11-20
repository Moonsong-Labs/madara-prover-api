use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("could not bind to Unix Domain Socket")]
    Io(#[from] std::io::Error),
    #[error("could not start server")]
    Transport(#[from] tonic::transport::Error),
}
