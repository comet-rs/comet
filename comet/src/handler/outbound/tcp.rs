use super::{NewOutboundHandler, Outbound, OutboundAddr, OutboundHandler};
use crate::config::OutboundTransportConfig;
use crate::prelude::*;
use crate::utils::metered_stream::MeteredStream;
use anyhow::anyhow;
use std::net::IpAddr;
use std::net::SocketAddr;
use tokio::io::BufReader;

pub struct TcpHandler {
    transport: OutboundTransportConfig,
    metering: bool,
}

impl TcpHandler {
    async fn connect(
        &self,
        tag: &str,
        addr: IpAddr,
        port: u16,
        ctx: &AppContextRef,
    ) -> Result<RWPair> {
        let stream = crate::net_wrapper::connect_tcp(&SocketAddr::from((addr, port))).await?;
        Ok(if self.metering {
            RWPair::new(MeteredStream::new_outbound(
                BufReader::new(stream),
                &tag,
                &ctx,
            ))
        } else {
            RWPair::new(BufReader::new(stream))
        })
    }
}

#[async_trait]
impl OutboundHandler for TcpHandler {
    async fn handle(
        &self,
        tag: &str,
        conn: &mut Connection,
        ctx: &AppContextRef,
    ) -> Result<ProxyStream> {
        let (ips, port) = self.resolve_addr(conn, ctx).await?;

        for ip in ips {
            match self.connect(tag, ip, port, ctx).await {
                Ok(stream) => return Ok(stream.into()),
                Err(err) => warn!("Trying {}:{} failed: {}", ip, port, err),
            }
        }
        Err(anyhow!("All attempts failed"))
    }

    fn port(&self) -> std::option::Option<u16> {
        self.transport.port
    }
    
    fn addr(&self) -> std::option::Option<&OutboundAddr> {
        self.transport.addr.as_ref()
    }
}

impl NewOutboundHandler for TcpHandler {
    fn new(config: &Outbound) -> Self {
        Self {
            transport: config.transport.clone(),
            metering: config.metering,
        }
    }
}
