fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(false)
        .compile(&["src/protos/gun/gun.proto"], &["src/protos/gun"])?;
    Ok(())
}
