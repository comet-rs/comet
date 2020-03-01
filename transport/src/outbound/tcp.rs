use crate::outbound::OutboundTransport;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use common::RWPair;
use std::net::SocketAddr;
use tokio::net::TcpStream;

pub struct OutboundTcpTransport;

#[async_trait]
impl OutboundTransport for OutboundTcpTransport {
    async fn connect(&self, addr: SocketAddr) -> Result<RWPair<'static>> {
        let stream = TcpStream::connect(&addr).await?;
        Ok(RWPair::new(stream))
    }
}
