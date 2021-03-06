use crate::{dns::DnsConfig, prelude::*, router::RouterConfig};
use anyhow::Result;
use serde::Deserialize;
use smol_str::SmolStr;
use std::net::IpAddr;
use std::{collections::HashMap, path::PathBuf};
use tokio::fs::{create_dir_all, File};

pub mod outbound;

pub use outbound::{Outbound, OutboundAddr, OutboundTransportType};

#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    #[serde(default = "default_current_dir")]
    pub data_dir: PathBuf,
    #[serde(default)]
    pub inbounds: HashMap<SmolStr, Inbound>,
    #[serde(default)]
    pub pipelines: HashMap<SmolStr, Vec<YamlValue>>,
    #[serde(default)]
    pub outbounds: HashMap<SmolStr, outbound::Outbound>,
    pub router: RouterConfig,
    #[cfg(target_os = "android")]
    pub android: AndroidConfig,
    #[serde(default)]
    pub rule_providers: HashMap<SmolStr, crate::rule_provider::ProviderConfig>,
    #[serde(default)]
    pub server_providers: HashMap<SmolStr, crate::server_provider::ProviderConfig>,
    #[serde(default)]
    pub dns: DnsConfig,
}

fn default_current_dir() -> PathBuf {
    let mut cwd = std::env::current_dir().unwrap();
    cwd.push("data");
    cwd
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

pub async fn load_file(path: &str) -> Result<Config> {
    let mut file = File::open(path).await?;
    let mut buffer = String::new();

    file.read_to_string(&mut buffer).await?;
    load_string(&buffer).await
}

pub async fn load_string(input: &str) -> Result<Config> {
    let config: Config = serde_yaml::from_str(input)?;

    create_dir_all(&config.data_dir).await?;

    Ok(config)
}
