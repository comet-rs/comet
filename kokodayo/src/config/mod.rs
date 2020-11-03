use crate::prelude::*;
use crate::processor;
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
  pub pipelines: HashMap<SmolStr, Vec<ProcessorConfig>>,
  #[serde(default)]
  pub outbounds: HashMap<SmolStr, Outbound>,
  pub router: RouterConfig,
  #[cfg(target_os = "android")]
  pub android: AndroidConfig
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
  pub transport: OutboundTransportConfig,
}

#[derive(Deserialize, Clone, Debug)]
pub struct OutboundTransportConfig {
  pub r#type: TransportType,
  pub port: Option<u16>,
  pub addr: Option<IpAddr>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AndroidConfig {
  pub ports: AndroidPorts
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

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all(deserialize = "snake_case"))]
#[serde(tag = "type")]
pub enum ProcessorConfig {
  Sniffer(processor::sniffer::SnifferConfig),

  Socks5ProxyServer(processor::socks5_proxy::Socks5ProxyServerConfig),
  // Socks5ProxyClient,

  HttpProxyServer(processor::http::server::ServerConfig),
  HttpProxyClient(processor::http::client::ClientConfig),

  ShadowsocksClient(processor::shadowsocks::ShadowsocksClientConfig),
  ShadowsocksClientHandshake(processor::shadowsocks::handshake::ShadowsocksClientHandshakeConfig),
  SsrClientAuth(processor::shadowsocks::auth::SsrClientAuthConfig),
  SsrClientObfs(processor::shadowsocks::obfs::ClientConfig),

  #[cfg(target_os = "android")]
  AndroidNat(processor::android::AndroidNatConfig),
  AssociateUid(processor::unix::AssociateUidConfig)
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouterConfig {
  pub rules: Vec<RouterRule>,
  pub defaults: RouterDefaults,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouterDefaults {
  pub tcp: SmolStr,
  pub udp: Option<SmolStr>
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
