use crate::config::{Config, Outbound, TransportType};
use crate::prelude::*;
use crate::utils::metered_stream::MeteredReader;
use crate::utils::metered_stream::MeteredWriter;
use std::borrow::Borrow;
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

  pub async fn connect_tcp<'a>(
    &'a self,
    tag: Option<&'a str> ,
    addr: impl Into<Option<IpAddr>>,
    port: impl Into<Option<u16>>,
    ctx: &AppContextRef,
  ) -> Result<(&'a str, RWPair)> {
    let tag_opt = Option::from(tag);
    let (tag, outbound) = match tag_opt {
      Some(tag) => (tag, self.outbounds.get(tag).unwrap()),
      None => self
        .outbounds
        .iter()
        .find(|o| o.1.transport.r#type == TransportType::Tcp)
        .map(|o| (o.0.borrow(), o.1))
        .unwrap(),
    };

    let port = outbound.transport.port.or_else(|| port.into()).unwrap();
    let addr = outbound.transport.addr.or_else(|| addr.into()).unwrap();

    let stream = crate::net_wrapper::connect_tcp(&SocketAddr::from((addr, port))).await?;
    let splitted = stream.into_split();

    Ok((tag, RWPair::new_parts(
      BufReader::new(MeteredReader::new_outbound(splitted.0, &tag, ctx)),
      MeteredWriter::new_outbound(splitted.1, &tag, ctx),
    )))
  }

  pub fn get_pipeline<'a>(
    &self,
    tag: impl Into<Option<&'a str>>,
    transport_type: TransportType,
  ) -> Option<&str> {
    let outbound = match tag.into() {
      Some(tag) => self.outbounds.get(tag.into()).unwrap(),
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
