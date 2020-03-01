use anyhow::Result;
use log::info;
use settings::Settings;
use tokio::stream::StreamExt;
pub mod manager;

pub struct GlobalContext {}

pub async fn run(settings: Settings) -> Result<()> {
    let inbound_manager = manager::InboundManager::new(settings.inbounds)?;
    let mut inbound_stream = inbound_manager.run().await?;

    loop {
        let conn = inbound_stream.next().await.unwrap();
        tokio::spawn(async move {
            if let Err(e) = socks5::proxy(conn).await {
                info!("Error: {}", e)
            }
        });
    }

    Ok(())
}
