use crate::config::Outbound;
use crate::prelude::*;
use std::net::IpAddr;

mod both;
mod dashboard;
mod tcp;
mod udp;
#[cfg(feature = "gun-transport")]
mod gun;

pub use both::TcpUdpHandler;
pub use dashboard::DashboardHandler;
pub use tcp::TcpHandler;
pub use udp::UdpHandler;
#[cfg(feature = "gun-transport")]
pub use gun::GunHandler;
#[cfg(feature = "gun-transport")]
pub use gun::GunConfig;

#[async_trait]
pub trait OutboundHandler: Send + Sync + Unpin {
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
        let port = conn.dest_addr.port_or_error()?;
        let ips = ctx.dns.resolve_addr(&conn.dest_addr, ctx).await?;
        Ok((ips, port))
    }
}

pub trait NewOutboundHandler {
    fn new(config: &Outbound) -> Self;
}
