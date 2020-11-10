use anyhow::Result;
use comet::run_bin;
use tokio::signal;

use log::info;
use log::LevelFilter;

#[tokio::main]
async fn main() -> Result<()> {
  env_logger::Builder::from_default_env()
    .filter(None, LevelFilter::Info)
    .init();
  run_bin().await?;
  info!("Service started, press Ctrl-C to stop");

  signal::ctrl_c().await?;
  info!("Ctrl-C received, stopping...");

  Ok(())
}
