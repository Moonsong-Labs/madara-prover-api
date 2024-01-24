use clap::Parser;
use std::path::PathBuf;

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
    evm_adapter::verify_with_l1(&args.annotated_proof, args.mainnet_rpc).await
}
