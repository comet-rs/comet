use crate::transport::StreamSettings;
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr};

#[derive(Deserialize, Default, Debug)]
pub struct InboundSocks5Settings {}

#[derive(Deserialize, Default, Debug)]
pub struct InboundAndroidSettings {
    fd: i32,
}

fn default_listen_addr() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "lowercase"))]
#[serde(tag = "protocol", content = "settings")]
pub enum InboundProtocolType {
    Socks(#[serde(default)] InboundSocks5Settings),
    Android(InboundAndroidSettings),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "lowercase"))]
pub enum DestOverrideType {
    Http,
    Tls,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct SniffingSettings {
    pub enabled: bool,
    pub dest_override: Vec<DestOverrideType>,
}

impl Default for SniffingSettings {
    fn default() -> Self {
        SniffingSettings {
            enabled: false,
            dest_override: Vec::new(),
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct InboundSettings {
    pub port: u16,
    #[serde(default = "default_listen_addr")]
    pub listen: IpAddr,

    #[serde(flatten)]
    pub protocol: InboundProtocolType, // Protocol + Settings
    #[serde(default)]
    pub stream_settings: StreamSettings,

    pub tag: Option<String>,
    #[serde(default)]
    pub sniffing: SniffingSettings,
}
