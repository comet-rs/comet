use anyhow::Result;
use settings::Settings;
mod manager;

pub fn run(settings: Settings) -> Result<()> {
    let inbound_manager = manager::InboundManager::new(settings.inbounds);

    Ok(())
}
