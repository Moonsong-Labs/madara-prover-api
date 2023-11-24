#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use rand::distributions::Alphanumeric;
    use rand::Rng;
    use rstest::{fixture, rstest};
    use tokio::net::UnixStream;
    use tokio::task::JoinHandle;
    use tonic::transport::{Endpoint, Uri};
    use tower::service_fn;

    use madara_prover_rpc_client::client::{execute_and_prove, execute_program, prove_execution};
    use madara_prover_rpc_client::prover::prover_client::ProverClient;
    use madara_prover_rpc_server::error::ServerError;
    use madara_prover_rpc_server::{run_grpc_server, BindAddress};
    use test_cases::get_test_case_file_path;
    use test_fixtures::{parsed_prover_test_case, prover_in_path, ParsedProverTestCase};

    type RpcClient = ProverClient<tonic::transport::Channel>;
    type RpcServer = JoinHandle<Result<(), ServerError>>;

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
    #[fixture]
    async fn rpc_client_server(#[from(prover_in_path)] _path: ()) -> (RpcClient, RpcServer) {
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

        let client = ProverClient::new(channel);
        (client, server_task)
    }

    #[rstest]
    #[tokio::test]
    async fn test_execute(#[future] rpc_client_server: (RpcClient, RpcServer)) {
        let (mut client, _server) = rpc_client_server.await;

        let program_path = get_test_case_file_path("fibonacci/fibonacci_compiled.json");
        let program_content = std::fs::read(program_path).unwrap();

        let result = execute_program(&mut client, program_content).await;

        assert!(result.is_ok(), "{:?}", result);
    }

    #[rstest]
    #[tokio::test]
    async fn test_prove(
        #[future] rpc_client_server: (RpcClient, RpcServer),
        #[from(parsed_prover_test_case)] test_case: ParsedProverTestCase,
    ) {
        let (mut client, _server) = rpc_client_server.await;

        let result = prove_execution(
            &mut client,
            test_case.public_input,
            test_case.memory,
            test_case.trace,
            test_case.prover_config,
            test_case.prover_parameters,
        )
        .await;

        assert!(result.is_ok(), "{:?}", result);

        let proof = result.unwrap();
        assert_eq!(proof.proof_hex, test_case.proof.proof_hex);
    }

    #[rstest]
    #[tokio::test]
    async fn test_execute_and_prove(
        #[future] rpc_client_server: (RpcClient, RpcServer),
        #[from(parsed_prover_test_case)] test_case: ParsedProverTestCase,
    ) {
        let (mut client, _server) = rpc_client_server.await;

        let result = execute_and_prove(
            &mut client,
            test_case.compiled_program,
            test_case.prover_config,
            test_case.prover_parameters,
        )
        .await;

        assert!(result.is_ok(), "{:?}", result);

        let proof = result.unwrap();
        assert_eq!(proof.proof_hex, test_case.proof.proof_hex);
    }
}
