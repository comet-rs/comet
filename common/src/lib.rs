pub use smallstr::SmallString;
use tokio::stream::Stream;
pub mod connection;
pub mod protocol;
mod rwpair;
pub use rwpair::RWPair;
use serde::Deserialize;
use std::fmt;
use std::net::IpAddr;
pub mod io;

pub type BoxedConnectionStream =
    Box<dyn Stream<Item = connection::Connection>>;

#[derive(Debug, Clone)]
pub enum Address {
    Domain(SmallString<[u8; 10]>),
    Ip(IpAddr),
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Address::Domain(s) => s.fmt(f),
            Address::Ip(a) => a.fmt(f),
        }
    }
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

impl fmt::Display for SocketAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.addr, self.port)
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "lowercase"))]
pub enum StreamType {
    Tcp,
}
impl Default for StreamType {
    fn default() -> Self {
        Self::Tcp
    }
}
