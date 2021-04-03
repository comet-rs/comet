use crate::config::{Config, OutboundTransportType};
use crate::handler::outbound::OutboundHandler;
use crate::prelude::*;
use anyhow::anyhow;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;

struct OutboundInstance {
    pipeline: Option<SmolStr>,
    handler: Box<dyn OutboundHandler>,
    timeout: u32,
}

pub struct OutboundManager {
    outbounds: HashMap<SmolStr, OutboundInstance>,
}

impl OutboundManager {
    pub fn new(config: &Config) -> Self {
        use crate::handler::outbound::*;

        let outbounds = config
            .outbounds
            .iter()
            .map(|(tag, outbound)| {
                let handler: Box<dyn OutboundHandler> = match outbound.typ {
                    OutboundTransportType::Tcp => Box::new(TcpHandler::new(outbound)),
                    OutboundTransportType::Udp => Box::new(UdpHandler::new(outbound)),
                    OutboundTransportType::Dashboard => Box::new(DashboardHandler::new(outbound)),
                    OutboundTransportType::TcpUdp => Box::new(TcpUdpHandler::new(outbound)),
                };
                let instance = OutboundInstance {
                    pipeline: outbound.pipeline.clone(),
                    timeout: outbound.timeout,
                    handler,
                };
                (tag.clone(), instance)
            })
            .collect();

        Self { outbounds }
    }

    pub async fn connect(
        &self,
        tag: &str,
        conn: &mut Connection,
        ctx: &AppContextRef,
    ) -> Result<ProxyStream> {
        let outbound = self.get_outbound(tag)?;
        let fut = outbound.handler.handle(tag, conn, ctx);

        if outbound.timeout > 0 {
            let dur = Duration::from_secs(outbound.timeout as u64);
            timeout(dur, fut).await?
        } else {
            fut.await
        }
    }

    pub fn get_pipeline(&self, tag: &str) -> Result<Option<&str>> {
        Ok(self
            .get_outbound(tag)?
            .pipeline
            .as_ref()
            .map(|r| r.as_str()))
    }

    fn get_outbound(&self, tag: &str) -> Result<&OutboundInstance> {
        let outbound = self
            .outbounds
            .get(tag)
            .ok_or_else(|| anyhow!("Outbound {} not found", tag))?;
        Ok(outbound)
    }
}
