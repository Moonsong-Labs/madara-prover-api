#[cfg(test)]
mod tests {
    type RpcClient = StarknetProverClient<tonic::transport::Channel>;

    use crate::integration::toolkit::{starknet_prover_client_server, RpcServer};
    use madara_prover_rpc_client::services::starknet_prover::execute_and_prove;
    use rstest::rstest;
    use madara_prover_common::models::Proof;
    use madara_prover_common::toolkit::read_json_from_file;
    use madara_prover_rpc_client::services::starknet_prover::starknet_prover_proto::starknet_prover_client::StarknetProverClient;
    use test_cases::get_test_case_file_path;

    #[rstest]
    #[tokio::test]
    async fn test_execute_and_prove(
        #[future] starknet_prover_client_server: (RpcClient, RpcServer),
    ) {
        let test_case_dir = get_test_case_file_path("bootloader/programs/fibonacci");
        let program_file = test_case_dir.join("program.json");
        let proof_file = test_case_dir.join("output/proof.json");
        let program_bytes = std::fs::read(program_file).unwrap();
        let expected_proof: Proof = read_json_from_file(proof_file).unwrap();

        let (mut client, _server) = starknet_prover_client_server.await;

        let programs = vec![program_bytes];
        let pies = vec![];
        let split_proof = false;

        let result = execute_and_prove(&mut client, programs, pies, split_proof).await;

        assert!(result.is_ok(), "{:?}", result);

        let proof = result.unwrap();
        assert_eq!(proof.proof_hex, expected_proof.proof_hex);
        assert!(proof.split_proofs.is_none());
    }

    #[ignore = "needs RPC URL"] // see "<redacted>" below
    #[rstest]
    #[tokio::test]
    async fn test_execute_and_prove_and_split(
        #[future] starknet_prover_client_server: (RpcClient, RpcServer),
    ) {
        let test_case_dir = get_test_case_file_path("bootloader/programs/fibonacci");
        let program_file = test_case_dir.join("program.json");
        let proof_file = test_case_dir.join("output/proof.json");
        let program_bytes = std::fs::read(program_file).unwrap();
        let expected_proof: Proof = read_json_from_file(proof_file).unwrap();

        let (mut client, _server) = starknet_prover_client_server.await;

        let programs = vec![program_bytes];
        let pies = vec![];
        let split_proof = true;

        let result = execute_and_prove(&mut client, programs, pies, split_proof).await;

        assert!(result.is_ok(), "{:?}", result);

        let proof = result.unwrap();
        assert_eq!(proof.proof_hex, expected_proof.proof_hex);

        assert!(proof.split_proofs.is_some());
        let split_proofs = proof.split_proofs.unwrap();
        assert!(split_proofs.merkle_statements.len() > 0);
        assert!(split_proofs.fri_merkle_statements.len() > 0);

        let private_url = "<redacted>";
        evm_adapter::verify_split_proofs_with_l1(&split_proofs, private_url.into()).await.unwrap();

    }

    #[ignore = "this test takes ~5 minutes to run"]
    #[rstest]
    #[tokio::test]
    async fn test_execute_and_prove_starknet_os(
        #[future] starknet_prover_client_server: (RpcClient, RpcServer),
    ) {
        let test_case_dir = get_test_case_file_path("starknet-os");
        let os_pie_file = test_case_dir.join("os.zip");
        let proof_file = test_case_dir.join("output/proof.json");
        let pie_bytes = std::fs::read(os_pie_file).unwrap();
        let expected_proof: Proof = read_json_from_file(proof_file).unwrap();

        let (mut client, _server) = starknet_prover_client_server.await;

        let programs = vec![];
        let pies = vec![pie_bytes];
        let split_proof = false;

        let result = execute_and_prove(&mut client, programs, pies, split_proof).await;

        assert!(result.is_ok(), "{:?}", result);

        let proof = result.unwrap();
        assert_eq!(proof.proof_hex, expected_proof.proof_hex);
    }
}
