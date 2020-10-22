use anyhow::Result;
use kokodayo::run;

use log::LevelFilter;

#[tokio::main]
async fn main() -> Result<()> {
  env_logger::Builder::from_default_env()
    .filter(None, LevelFilter::Debug)
    .init();
  run().await?;
  Ok(())
}
