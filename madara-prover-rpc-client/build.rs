fn main() -> Result<(), Box<dyn std::error::Error>> {
    let builder = tonic_build::configure()
        .protoc_arg("--experimental_allow_proto3_optional")
        .build_server(false);
    builder.compile(
        &[
            "../protocols/prover.proto",
            "../protocols/starknet_prover.proto",
        ],
        &["../protocols"],
    )?;
    Ok(())
}
