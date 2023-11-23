fn main() -> Result<(), Box<dyn std::error::Error>> {
    let builder = tonic_build::configure()
        .protoc_arg("--experimental_allow_proto3_optional")
        .build_client(true);
    builder.compile(&["../protocols/prover.proto"], &["../protocols"])?;
    Ok(())
}
