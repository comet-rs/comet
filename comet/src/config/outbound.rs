use std::net::IpAddr;

use serde::Deserialize;
use smol_str::SmolStr;

#[derive(Deserialize, Clone, Debug)]
pub struct Outbound {
    pub pipeline: Option<SmolStr>,
    #[serde(default)]
    pub metering: bool,
    #[serde(default)]
    pub timeout: u32,
    #[serde(flatten)]
    pub typ: OutboundTransportType,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboundTransportType {
    Tcp,
    Udp,
    #[serde(rename = "tcp+udp")]
    TcpUdp,
    Dashboard,
    #[cfg(feature = "gun-transport")]
    Gun {
        config: crate::handler::outbound::GunConfig,
    },
}

impl Default for OutboundTransportType {
    fn default() -> Self {
        Self::TcpUdp
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum OutboundAddr {
    Ip(IpAddr),
    Domain(SmolStr),
}
