use anyhow::Result;
use comet::run_bin;
use tokio::signal;

use log::info;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );
    run_bin().await?;
    info!("Service started, press Ctrl-C to stop");

    signal::ctrl_c().await?;
    info!("Ctrl-C received, stopping...");

    Ok(())
}
