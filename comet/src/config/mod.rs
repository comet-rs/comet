use crate::prelude::*;
use crate::router::matching::MatchCondition;
use anyhow::Result;
use serde::Deserialize;
use smol_str::SmolStr;
use std::collections::HashMap;
use std::net::IpAddr;
use tokio::fs::File;
use tokio::prelude::*;

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
  #[serde(default)]
  pub inbounds: HashMap<SmolStr, Inbound>,
  #[serde(default)]
  pub pipelines: HashMap<SmolStr, Vec<YamlValue>>,
  #[serde(default)]
  pub outbounds: HashMap<SmolStr, Outbound>,
  pub router: RouterConfig,
  #[cfg(target_os = "android")]
  pub android: AndroidConfig,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Inbound {
  pub pipeline: Option<SmolStr>,
  #[serde(default)]
  pub metering: bool,
  pub transport: InboundTransportConfig,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InboundTransportType {
  Tcp,
  Udp,
}

#[derive(Deserialize, Clone, Debug)]
pub struct InboundTransportConfig {
  #[serde(flatten)]
  pub r#type: InboundTransportType,
  pub port: u16,
  pub listen: Option<IpAddr>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Outbound {
  pub pipeline: Option<SmolStr>,
  #[serde(default)]
  pub metering: bool,
  #[serde(default)]
  pub timeout: u32,
  pub transport: OutboundTransportConfig,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboundTransportType {
  Tcp,
  Udp,
  Dashboard,
}

#[derive(Deserialize, Clone, Debug)]
pub struct OutboundTransportConfig {
  #[serde(flatten)]
  pub r#type: OutboundTransportType,
  pub port: Option<u16>,
  pub addr: Option<OutboundAddr>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum OutboundAddr {
  Ip(IpAddr),
  Domain(SmolStr),
}

#[derive(Deserialize, Clone, Debug)]
pub struct AndroidConfig {
  pub ports: AndroidPorts,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AndroidPorts {
  pub tcp: u16,
  pub tcp_v6: Option<u16>,
  pub udp: u16,
  pub udp_v6: Option<u16>,
  pub dns: u16,
  pub dns_v6: Option<u16>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouterConfig {
  pub rules: Vec<RouterRule>,
  pub defaults: RouterDefaults,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouterDefaults {
  pub tcp: SmolStr,
  pub udp: Option<SmolStr>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouterRule {
  pub target: SmolStr,
  pub rule: MatchCondition,
}

pub async fn load_file(path: &str) -> Result<Config> {
  let mut file = File::open(path).await?;
  let mut buffer = String::new();

  file.read_to_string(&mut buffer).await?;
  Ok(serde_yaml::from_str(&buffer)?)
}

pub fn load_string(input: &str) -> Result<Config> {
  Ok(serde_yaml::from_str(input)?)
}
