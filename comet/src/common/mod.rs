use anyhow::{anyhow, Result};
use serde::Deserialize;

mod connection;
mod context;
mod packet;
mod rwpair;

pub use connection::{Connection, DestAddr};
pub use context::{AppContext, AppContextRef};
pub use packet::UdpStream;
pub use rwpair::RWPair;

use std::sync::Arc;
use tokio::net::UdpSocket;

#[derive(Debug)]
pub enum ProxyStream {
    Tcp(RWPair),
    Udp(UdpStream),
}

impl ProxyStream {
    pub fn into_tcp(self) -> Result<RWPair> {
        match self {
            Self::Tcp(s) => Ok(s),
            Self::Udp(_) => Err(anyhow!("Incompatible type: UDP")),
        }
    }

    pub fn into_udp(self) -> Result<UdpStream> {
        match self {
            Self::Tcp(_) => Err(anyhow!("Incompatible type: TCP")),
            Self::Udp(s) => Ok(s),
        }
    }
}

impl From<RWPair> for ProxyStream {
    fn from(s: RWPair) -> Self {
        Self::Tcp(s)
    }
}
impl From<UdpStream> for ProxyStream {
    fn from(s: UdpStream) -> Self {
        Self::Udp(s)
    }
}

pub enum OutboundStream {
    Tcp(RWPair),
    Udp(Arc<UdpSocket>),
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all(deserialize = "snake_case"))]
pub enum TransportType {
    Tcp,
    Udp,
}

impl std::fmt::Display for TransportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        let s = match self {
            TransportType::Tcp => "TCP",
            TransportType::Udp => "UDP",
        };
        write!(f, "{}", s)
    }
}

