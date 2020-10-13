mod tcp;
use anyhow::Result;
use async_trait::async_trait;
use common::connection::InboundConnection;
use common::StreamType;
use settings::inbound::InboundSettings;
use tcp::InboundTcpTransport;

#[async_trait]
pub trait InboundTransport: Send {
    // async fn listen(settings: &InboundSettings) -> Result<Self>;
    async fn accept(&mut self) -> Result<InboundConnection>;
}

pub async fn create_transport(settings: &InboundSettings) -> Result<Box<dyn InboundTransport>> {
    let ret = match settings.stream_settings.network {
        StreamType::Tcp => Box::new(InboundTcpTransport::listen(&settings).await?),
    };
    Ok(ret)
}
