use super::{NewOutboundHandler, OutboundHandler, TcpHandler, UdpHandler};
use crate::{config::Outbound, prelude::*};

pub struct TcpUdpHandler {
    tcp: TcpHandler,
    udp: UdpHandler,
}

#[async_trait]
impl OutboundHandler for TcpUdpHandler {
    async fn handle(
        &self,
        tag: &str,
        conn: &mut Connection,
        ctx: &AppContextRef,
    ) -> Result<ProxyStream> {
        match conn.typ {
            TransportType::Tcp => self.tcp.handle(tag, conn, ctx).await,
            TransportType::Udp => self.udp.handle(tag, conn, ctx).await,
        }
    }
}

impl NewOutboundHandler for TcpUdpHandler {
    fn new(config: &Outbound) -> Self {
        Self {
            tcp: TcpHandler::new(config),
            udp: UdpHandler::new(config),
        }
    }
}
