use crate::config::{Config, OutboundTransportType};
use crate::handler::outbound::OutboundHandler;
use crate::prelude::*;
use anyhow::anyhow;
use std::collections::HashMap;

struct OutboundInstance {
  pipeline: Option<SmolStr>,
  handler: Box<dyn OutboundHandler>,
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
        let handler: Box<dyn OutboundHandler> = match outbound.transport.r#type {
          OutboundTransportType::Tcp => Box::new(TcpHandler::new(outbound)),
          OutboundTransportType::Udp => Box::new(UdpHandler::new(outbound)),
          OutboundTransportType::Dashboard => Box::new(DashboardHandler::new(outbound)),
        };
        let instance = OutboundInstance {
          pipeline: outbound.pipeline.clone(),
          handler: handler,
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
    outbound.handler.handle(tag, conn, ctx).await
  }

  pub fn get_pipeline(&self, tag: &str) -> Result<Option<&str>> {
    Ok(match self.get_outbound(tag)?.pipeline.as_ref() {
      Some(r) => Some(r),
      None => None,
    })
  }

  fn get_outbound(&self, tag: &str) -> Result<&OutboundInstance> {
    let outbound = self
      .outbounds
      .get(tag)
      .ok_or_else(|| anyhow!("Outbound {} not found", tag))?;
    Ok(outbound)
  }
}
