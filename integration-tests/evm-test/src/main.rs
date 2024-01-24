use clap::Parser;
use ethers::{
    core::k256::ecdsa::SigningKey,
    middleware::SignerMiddleware,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::{Address, U256, U64},
    utils::{hex, Anvil},
};
use stark_evm_adapter::{
    annotated_proof::{self, AnnotatedProof},
    annotation_parser::{split_fri_merkle_statements, SplitProofs},
    ContractFunctionCall,
};
use std::{
    convert::TryFrom, env, fs, path::PathBuf, str::FromStr, sync::Arc
};

/// Binary borrowed from `stark-evm-adapter` used to test a split proof against in-production
/// SHARP provers on Ethereum.
/// 
/// Source: https://github.com/notlesh/stark-evm-adapter/blob/main/examples/verify_stone_proof.rs
/// 
/// Input file ("split proof") should be a proof JSON file generated from `cpu_air_prover` along
/// with an `annotations` field (array) and `extra_annotations` field (array) which come from,
/// respectively, `--annotations_file` and `--extra_output_file` from `cpu_air_verifier`.
/// 
/// This also requires `anvil` from `forge`
/// [to be installed](https://book.getfoundry.sh/getting-started/installation).
/// 
/// A suitable input file can be borrowed from
/// https://github.com/notlesh/stark-evm-adapter/blob/main/tests/fixtures/annotated_proof.json

// CLI Args
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    #[arg(short, long)]
    annotated_proof: PathBuf,

    // TODO: support FORKED_MAINNET_RPC and set up proper arg group
    #[arg(short, long, required = true)]
    mainnet_rpc: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let proof_str = fs::read_to_string(args.annotated_proof)?;
    let annotated_proof: AnnotatedProof = serde_json::from_str(proof_str.as_str())?;

    let mut anvil = None;

    let url = args.mainnet_rpc;
    anvil = Some(Anvil::new().fork(url).spawn());
    let endpoint = anvil.as_ref().unwrap().endpoint();
    let provider = Provider::<Http>::try_from(endpoint.as_str())?;

    // a trick to make anvil process lives in the whole main function
    if anvil.is_some() {
        println!("Anvil is running.");
    }

    // test private key from anvil node
    let from_key_bytes =
        hex::decode("0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d").unwrap();

    let from_signing_key = SigningKey::from_bytes(from_key_bytes.as_slice().into()).unwrap();
    let from_wallet: LocalWallet = LocalWallet::from(from_signing_key);
    println!("Test wallet address: {:?}", from_wallet.address());

    let chain_id = provider.get_chainid().await?.as_u32();
    let signer: Arc<SignerMiddleware<_, _>> = Arc::new(SignerMiddleware::new(
        provider.clone(),
        from_wallet.with_chain_id(chain_id),
    ));

    // generate split proofs
    let split_proofs: SplitProofs = split_fri_merkle_statements(annotated_proof.clone()).unwrap();

    // start verifying all split proofs
    println!("Verifying trace decommitments:");
    let contract_address = Address::from_str("0x5899Efea757E0Dbd6d114b3375C23D7540f65fa4").unwrap();
    for i in 0..split_proofs.merkle_statements.len() {
        let key = format!("Trace {}", i);
        let trace_merkle = split_proofs.merkle_statements.get(&key).unwrap();

        let call = trace_merkle.verify(contract_address, signer.clone());

        assert_call(call, &key).await?;
    }

    println!("Verifying FRI decommitments:");
    let contract_address = Address::from_str("0x3E6118DA317f7A433031F03bB71ab870d87dd2DD").unwrap();
    for (i, fri_statement) in split_proofs.fri_merkle_statements.iter().enumerate() {
        let call = fri_statement.verify(contract_address, signer.clone());

        assert_call(call, &format!("FRI statement: {}", i)).await?;
    }

    println!("Verifying main proof:");
    let contract_address = Address::from_str("0x47312450B3Ac8b5b8e247a6bB6d523e7605bDb60").unwrap();

    let task_metadata = vec![U256::zero()];
    let call = split_proofs
        .main_proof
        .verify(contract_address, signer, task_metadata);

    assert_call(call, "Main proof").await?;

    Ok(())
}

async fn assert_call(
    call: ContractFunctionCall,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let pending_tx = call.send().await?;
    let mined_tx = pending_tx.await?;
    assert_eq!(
        U64::from(1),
        mined_tx.unwrap().status.unwrap(),
        "tx failed: {}",
        name
    );
    println!("Verified: {}", name);
    Ok(())
}
