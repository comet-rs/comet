use crate::outbound::OutboundTransport;
use anyhow::Result;
use async_trait::async_trait;
use common::RWPair;
use net_wrapper::connect_tcp;
use std::net::SocketAddr;

pub struct OutboundTcpTransport;

#[async_trait]
impl OutboundTransport for OutboundTcpTransport {
    async fn connect(&self, addr: SocketAddr) -> Result<RWPair> {
        let stream = connect_tcp(&addr).await?;
        Ok(RWPair::new(stream))
    }
}
