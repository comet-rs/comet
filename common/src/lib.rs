pub use smallstr::SmallString;
pub mod connection;
pub mod protocol;
mod rwpair;
pub use rwpair::RWPair;
use serde::Deserialize;
use std::net::IpAddr;

#[derive(Debug, Clone)]
pub enum Address {
    Domain(SmallString<[u8; 10]>),
    Ip(IpAddr),
}

#[derive(Debug, Clone)]
pub struct SocketAddress {
    pub addr: Address,
    pub port: u16,
}

impl SocketAddress {
    pub fn new(addr: Address, port: u16) -> SocketAddress {
        SocketAddress {
            addr: addr,
            port: port,
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "lowercase"))]
pub enum NetworkType {
    Tcp,
    Udp,
}
impl Default for NetworkType {
    fn default() -> Self {
        Self::Tcp
    }
}
