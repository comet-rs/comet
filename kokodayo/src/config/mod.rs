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
  HttpProxyClient,

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

pub async fn load_file(path: &str) -> Result<Config> {
  let mut file = File::open(path).await?;
  let mut buffer = String::new();

  file.read_to_string(&mut buffer).await?;
  Ok(serde_yaml::from_str(&buffer)?)
}