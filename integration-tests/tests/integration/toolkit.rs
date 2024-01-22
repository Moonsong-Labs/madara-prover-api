use madara_prover_rpc_client::services::prover::prover_proto::prover_client::ProverClient;
use madara_prover_rpc_client::services::starknet_prover::starknet_prover_proto::starknet_prover_client::StarknetProverClient;
use madara_prover_rpc_server::error::ServerError;
use madara_prover_rpc_server::{run_grpc_server, BindAddress};
use rand::distributions::Alphanumeric;
use rand::Rng;
use rstest::fixture;
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::task::JoinHandle;
use tonic::transport::{Endpoint, Uri};
use tower::service_fn;
use test_fixtures::prover_in_path;

pub type RpcServer = JoinHandle<Result<(), ServerError>>;

fn random_string(length: usize) -> String {
    (0..length)
        .map(|_| rand::thread_rng().sample(Alphanumeric) as char)
        .collect()
}

fn generate_socket_path() -> PathBuf {
    let filename = format!("/tmp/{}.sock", random_string(8));
    PathBuf::from(filename)
}

/// Starts an RPC server and client and returns them both.
///
/// The client and server communicate over a Unix socket.
async fn rpc_client_server<T>(
    client_factory: fn(tonic::transport::Channel) -> T,
) -> (T, RpcServer) {
    let unix_socket_client = generate_socket_path();
    let unix_socket_server = unix_socket_client.clone();

    let server_task = tokio::spawn(async move {
        run_grpc_server(BindAddress::UnixSocket(unix_socket_server.as_path())).await
    });

    // TODO: attempt to declare the client until the server responds instead of this (slow) sleep
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Note that the URI parameter is ignored.
    let channel = Endpoint::try_from("http://[::]:65535")
        .unwrap()
        .connect_with_connector(service_fn(move |_: Uri| {
            UnixStream::connect(unix_socket_client.clone())
        }))
        .await
        .unwrap();

    let client = client_factory(channel);
    (client, server_task)
}

#[fixture]
pub async fn prover_client_server(
    #[from(prover_in_path)] _path: (),
) -> (ProverClient<tonic::transport::Channel>, RpcServer) {
    rpc_client_server(ProverClient::new).await
}

#[fixture]
pub async fn starknet_prover_client_server(
    #[from(prover_in_path)] _path: (),
) -> (StarknetProverClient<tonic::transport::Channel>, RpcServer) {
    rpc_client_server(StarknetProverClient::new).await
}
