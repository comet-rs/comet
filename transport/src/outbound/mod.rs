mod tcp;
pub use tcp::OutboundTcpTransport;

use anyhow::Result;
use async_trait::async_trait;
use common::RWPair;
use std::net::SocketAddr;

#[async_trait]
pub trait OutboundTransport {
    async fn connect(&self, addr: SocketAddr) -> Result<RWPair>;
}
