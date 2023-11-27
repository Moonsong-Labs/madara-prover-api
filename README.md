# Madara Prover API

RPC server and client to run the Stone Prover on the Madara sequencer.

## Description

This project provides a server that can run any set of Cairo programs on top of the Starknet OS to generate the state diff and proof of execution of the programs.
This server is used as part of the Madara sequencer to prove the transactions inside each (Madara) block.

To prove transactions, the server relies on the [Stone prover](https://github.com/starkware-libs/stone-prover).

This project is made of 3 main crates.
`stone-prover` provides a Rust wrapper for the Stone prover. This crate allows any Rust program to call the prover.
`madara-prover-rpc-server` contains the server code. This crate provides a binary and library to spawn the server. 
`madara-prover-rpc-client` provides a client to interact with the server.

The client and server communicate using [gRPC](https://grpc.io/). 
You can find the protocol description in `protocols/prover.proto`.

## Usage

### Prove and execute a Cairo program

```rust
use madara_prover_rpc_client::client::execute_and_prove;
use madara_prover_rpc_client::prover::prover_client::ProverClient;

pub async fn prove_cairo_program() -> Result((), Box<dyn Error>) {
    let mut client = ProverClient::connect("http://[::1]:10000").await?;
    
    let compiled_program = std::fs::read("cairo_program_compiled.json")?;
    let prover_config = ProverConfig {...};
    let prover_parameters = ProverParameters {...};
    
    let proof = execute_and_prove(&mut client, compiled_program, prover_config, prover_parameters).await?;
    println!("Proof: {}", proof.proof_hex);
}

```

## Project structure

* `integration-tests`: Integration tests.
* `madara-prover-common`: Types and functions used in both the server and client.
* `madara-prover-rpc-client`: Prover API client.
* `madara-prover-rpc-server`: Prover API server.
* `protocols`: Protocol buffers are stored here.
* `stone-prover`: Rust wrapper for the Stone prover.
* `test-cases`: Cairo programs used as test cases.
* `test-fixtures`: Shared test fixtures and utilities.

## Getting started

### Build and test the project

The project can be built and tested using `cargo`.

First, clone the repository and its submodules:

```shell
git clone --recursive https://github.com/Moonsong-Labs/madara-prover-api.git 
```

Then, install dependencies:

```shell
sudo apt install protobuf-compiler  # Ubuntu
brew install protobuf   # macOS
```

Note that building this project requires Docker.
The Stone prover is built using a Dockerfile.

Once all dependencies are installed, you can simply run `cargo build` to build the project.

> The first build takes ~10 minutes because compiling the Stone prover takes a while.



