use std::path::Path;

use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

use crate::error::ServerError;
use crate::services::prover::prover_proto::prover_server::ProverServer;
use crate::services::prover::ProverService;
use crate::services::starknet_prover::starknet_prover_proto::starknet_prover_server::StarknetProverServer;
use crate::services::starknet_prover::StarknetProverService;

pub mod cairo;
pub mod error;
pub mod evm_adapter;
pub mod services;

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
