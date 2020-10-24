use crate::config::{Config, Outbound};
use crate::prelude::*;
use crate::utils::metered_stream::MeteredReader;
use crate::utils::metered_stream::MeteredWriter;
use std::collections::HashMap;
use std::net::IpAddr;
use std::net::SocketAddr;
use tokio::io::BufReader;

pub struct OutboundManager {
  outbounds: HashMap<SmolStr, Outbound>,
}

impl OutboundManager {
  pub fn new(config: &Config) -> Self {
    Self {
      outbounds: config.outbounds.clone(),
    }
  }

  pub async fn connect_tcp(
    &self,
    tag: &str,
    addr: impl Into<Option<IpAddr>>,
    port: impl Into<Option<u16>>,
    ctx: &AppContextRef,
  ) -> Result<RWPair> {
    let outbound = self.outbounds.get(tag).unwrap();

    let port = outbound.transport.port.or_else(|| port.into()).unwrap();
    let addr = outbound.transport.addr.or_else(|| addr.into()).unwrap();

    let stream = crate::net_wrapper::connect_tcp(&SocketAddr::from((addr, port))).await?;
    let splitted = stream.into_split();

    Ok(RWPair::new_parts(
      BufReader::new(MeteredReader::new_outbound(splitted.0, &tag, ctx)),
      MeteredWriter::new_outbound(splitted.1, &tag, ctx),
    ))
  }

  pub fn get_pipeline<'a>(
    &self,
    tag: impl Into<Option<&'a str>>,
    transport_type: TransportType,
  ) -> Option<&str> {
    let outbound = match tag.into() {
      Some(tag) => self.outbounds.get(tag).unwrap(),
      None => {
        self
          .outbounds
          .iter()
          .find(|o| o.1.transport.r#type == transport_type)
          .unwrap()
          .1
      }
    };

    assert!(outbound.transport.r#type == transport_type);

    match outbound.pipeline.as_ref() {
      Some(r) => Some(r),
      None => None,
    }
  }
}
