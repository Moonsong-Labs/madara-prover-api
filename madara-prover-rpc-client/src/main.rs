use crate::client::call_prover;
use prover::prover_client::ProverClient;
use std::path::Path;

pub mod client;
mod prover;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ProverClient::connect("http://[::1]:8080").await?;

    let fixtures_dir = Path::new("../stone-prover/tests/fixtures/fibonacci");
    let public_input =
        std::fs::read_to_string(fixtures_dir.join("fibonacci_public_input.json")).unwrap();
    let memory = std::fs::read(fixtures_dir.join("fibonacci_memory.bin")).unwrap();
    let trace = std::fs::read(fixtures_dir.join("fibonacci_trace.bin")).unwrap();
    let prover_config =
        std::fs::read_to_string(fixtures_dir.join("cpu_air_prover_config.json")).unwrap();
    let prover_parameters =
        std::fs::read_to_string(fixtures_dir.join("cpu_air_params.json")).unwrap();

    let response = call_prover(
        &mut client,
        public_input,
        memory,
        trace,
        prover_config,
        prover_parameters,
    )
    .await?;
    println!("Got: '{}' from service", response.proof_hex);
    Ok(())
}
