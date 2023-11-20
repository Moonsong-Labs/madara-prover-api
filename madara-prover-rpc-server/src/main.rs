use madara_prover_rpc_server::{run_grpc_server, BindAddress};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket_addr: SocketAddr = "[::1]:8080".parse().unwrap();
    run_grpc_server(BindAddress::Tcp(socket_addr)).await?;

    Ok(())
}
