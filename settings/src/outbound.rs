use crate::transport::StreamSettings;
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr};

fn default_send_addr() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))
}

pub mod protocols {
    use serde::Deserialize;
    #[derive(Deserialize, Default, Debug)]
    pub struct OutboundVmessSettings {}

    #[derive(Deserialize, Default, Debug)]
    #[serde(rename_all(deserialize = "camelCase"))]
    pub struct OutboundFreedomSettings {
        #[serde(default)]
        domain_strategy: FreedomDomainStrategy,
    }
    
    #[derive(Deserialize, Debug)]
    pub enum FreedomDomainStrategy {
        AsIs,
        UseIP,
        UseIPv4,
        UseIPv6,
    }
    impl Default for FreedomDomainStrategy {
        fn default() -> Self {
            Self::AsIs
        }
    }

    #[derive(Deserialize, Default, Debug)]
    pub struct OutboundBlackholeSettings {}
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "lowercase"))]
#[serde(tag = "protocol", content = "settings")]
pub enum OutboundProtocolType {
    Vmess(protocols::OutboundVmessSettings),
    Freedom(#[serde(default)] protocols::OutboundFreedomSettings),
    Blackhole(#[serde(default)] protocols::OutboundBlackholeSettings),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct OutboundSettings {
    #[serde(default = "default_send_addr")]
    pub send_through: IpAddr,

    #[serde(flatten)]
    pub protocol: OutboundProtocolType, // Protocol + Settings
    #[serde(default)]
    pub stream_settings: StreamSettings,

    pub tag: Option<String>,
}
