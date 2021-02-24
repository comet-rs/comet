use super::{NewOutboundHandler, Outbound, OutboundHandler};
use crate::prelude::*;
use crate::utils::metered_stream::MeteredStream;
use anyhow::anyhow;
use std::net::IpAddr;
use std::net::SocketAddr;
use tokio::io::BufReader;

pub struct TcpHandler {
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
}

impl NewOutboundHandler for TcpHandler {
    fn new(config: &Outbound) -> Self {
        Self {
            metering: config.metering,
        }
    }
}
