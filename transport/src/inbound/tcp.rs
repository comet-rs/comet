use crate::inbound::InboundTransport;
use anyhow::Result;
use async_trait::async_trait;
use common::connection::InboundConnection;
use common::RWPair;
use log::info;
use settings::inbound::InboundSettings;
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub struct InboundTcpTransport {
    listener: TcpListener,
}

impl InboundTcpTransport {
    pub async fn listen(settings: &InboundSettings) -> Result<Self> {
        let addr = SocketAddr::new(settings.listen, settings.port);
        let ret = InboundTcpTransport {
            listener: TcpListener::bind(&addr).await?,
        };
        info!("TCP listening: {}", addr);
        Ok(ret)
    }
}

#[async_trait]
impl InboundTransport for InboundTcpTransport {
    async fn accept(&mut self) -> Result<InboundConnection<'static>> {
        let (socket, addr) = self.listener.accept().await?;
        info!("{} accepted from {}", self.listener.local_addr()?, addr);
        Ok(InboundConnection {
            addr: addr,
            conn: RWPair::new(socket),
        })
    }
}
