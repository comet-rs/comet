use crate::config::Outbound;
use crate::config::OutboundAddr;
use crate::prelude::*;
use std::net::IpAddr;

mod both;
mod dashboard;
mod tcp;
mod udp;

pub use both::TcpUdpHandler;
pub use dashboard::DashboardHandler;
pub use tcp::TcpHandler;
pub use udp::UdpHandler;

#[async_trait]
pub trait OutboundHandler: Send + Sync + Unpin {
    fn port(&self) -> Option<u16>;
    fn addr(&self) -> Option<&OutboundAddr>;
    async fn handle(
        &self,
        tag: &str,
        conn: &mut Connection,
        ctx: &AppContextRef,
    ) -> Result<ProxyStream>;
    async fn resolve_addr(
        &self,
        conn: &Connection,
        ctx: &AppContextRef,
    ) -> Result<(Vec<IpAddr>, u16)> {
        let port = if let Some(port) = self.port() {
            port
        } else {
            conn.dest_addr.port_or_error()?
        };

        let ips = if let Some(addr) = self.addr() {
            // Dest addr overridden
            match addr {
                OutboundAddr::Ip(ip) => vec![*ip],
                OutboundAddr::Domain(domain) => ctx.dns.resolve(&domain).await?,
            }
        } else {
            ctx.dns.resolve_addr(&conn.dest_addr).await?
        };

        Ok((ips, port))
    }
}

pub trait NewOutboundHandler {
    fn new(config: &Outbound) -> Self;
}
