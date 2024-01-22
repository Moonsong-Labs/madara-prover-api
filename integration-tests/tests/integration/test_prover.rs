#[cfg(test)]
mod tests {
    use rstest::rstest;

    use madara_prover_rpc_client::services::prover::prover_proto::prover_client::ProverClient;
    use madara_prover_rpc_client::services::prover::{
        execute_and_prove, execute_program, prove_execution,
    };
    use test_cases::get_test_case_file_path;
    use test_fixtures::{parsed_prover_test_case, ParsedProverTestCase};

    use crate::integration::toolkit::{prover_client_server, RpcServer};

    type RpcClient = ProverClient<tonic::transport::Channel>;

    #[rstest]
    #[tokio::test]
    async fn test_execute(#[future] prover_client_server: (RpcClient, RpcServer)) {
        let (mut client, _server) = prover_client_server.await;

        let program_path = get_test_case_file_path("fibonacci/fibonacci_compiled.json");
        let program_content = std::fs::read(program_path).unwrap();

        let result = execute_program(&mut client, program_content).await;

        assert!(result.is_ok(), "{:?}", result);
    }

    #[rstest]
    #[tokio::test]
    async fn test_prove(
        #[future] prover_client_server: (RpcClient, RpcServer),
        #[from(parsed_prover_test_case)] test_case: ParsedProverTestCase,
    ) {
        let (mut client, _server) = prover_client_server.await;

        let result = prove_execution(
            &mut client,
            test_case.public_input,
            test_case.private_input,
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
    #[case(false)]
    #[case(true)]
    #[tokio::test]
    async fn test_execute_and_prove(
        #[future] prover_client_server: (RpcClient, RpcServer),
        #[from(parsed_prover_test_case)] test_case: ParsedProverTestCase,
        #[case] provide_prover_config_and_parameters: bool,
    ) {
        let (mut client, _server) = prover_client_server.await;

        let (prover_config, prover_parameters) = match provide_prover_config_and_parameters {
            true => (
                Some(test_case.prover_config),
                Some(test_case.prover_parameters),
            ),
            false => (None, None),
        };

        let result = execute_and_prove(
            &mut client,
            test_case.compiled_program,
            prover_config,
            prover_parameters,
        )
        .await;

        assert!(result.is_ok(), "{:?}", result);

        let proof = result.unwrap();
        assert_eq!(proof.proof_hex, test_case.proof.proof_hex);
    }
}
