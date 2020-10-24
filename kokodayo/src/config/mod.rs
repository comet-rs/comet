use crate::router::matching::MatchCondition;
use std::net::IpAddr;
use smol_str::SmolStr;
use crate::processor;
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use tokio::fs::File;
use tokio::prelude::*;

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
  #[serde(default)]
  pub inbounds: HashMap<SmolStr, Inbound>,
  #[serde(default)]
  pub pipelines: HashMap<SmolStr, Vec<ProcessorConfig>>,
  #[serde(default)]
  pub outbounds: HashMap<SmolStr, Outbound>,
  #[serde(default)]
  pub router: RouterConfig
}

#[derive(Deserialize, Clone, Debug)]
pub struct Inbound {
  pub pipeline: SmolStr,
  pub transport: TransportConfig,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TransportConfig {
  pub r#type: TransportType,
  pub port: u16,
  pub listen: Option<IpAddr>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Outbound {
  pub pipeline: Option<SmolStr>,
  pub transport: OutboundTransportConfig
}

#[derive(Deserialize, Clone, Debug)]
pub struct OutboundTransportConfig {
  pub r#type: TransportType,
  pub port: Option<u16>,
  pub addr: Option<IpAddr>
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all(deserialize = "snake_case"))]
pub enum TransportType {
  Tcp,
  Udp,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all(deserialize = "snake_case"))]
#[serde(tag = "type")]
pub enum ProcessorConfig {
  Sniffer(processor::sniffer::SnifferConfig),

  Socks5ProxyServer(processor::socks5_proxy::Socks5ProxyServerConfig),
  Socks5ProxyClient,

  HttpProxyServer,
  HttpProxyClient(processor::http_proxy::HttpProxyClientConfig),

  ShadowsocksServer,
  ShadowsocksClient,

  SsrObfsServer,
  SsrObfsClient,

  VmessServer,
  VmessClient,

  Switch {
    cases: Vec<processor::switch::SwitchCase>,
  },
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct RouterConfig {
  pub rules: Vec<RouterRule>
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouterRule {
  pub target: SmolStr,
  pub rule: MatchCondition
}

pub async fn load_file(path: &str) -> Result<Config> {
  let mut file = File::open(path).await?;
  let mut buffer = String::new();

  file.read_to_string(&mut buffer).await?;
  Ok(serde_yaml::from_str(&buffer)?)
}