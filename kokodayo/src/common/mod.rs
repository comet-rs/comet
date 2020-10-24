use smol_str::SmolStr;
use std::net::SocketAddr;
use tokio::stream::Stream;
pub mod connection;
mod rwpair;
pub use rwpair::RWPair;
use serde::Deserialize;
use std::fmt;
use std::net::IpAddr;
pub mod io;
pub use connection::Connection;
pub type BoxedConnectionStream = Box<dyn Stream<Item = connection::Connection>>;

#[derive(Debug, Clone)]
pub enum Address {
    Domain(SmolStr),
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
pub struct SocketDomainAddr {
    pub addr: Address,
    pub port: u16,
}

impl SocketDomainAddr {
    pub fn new(addr: Address, port: u16) -> SocketDomainAddr {
        SocketDomainAddr { addr, port }
    }

    pub fn new_domain<T: Into<SmolStr>>(addr: T, port: u16) -> SocketDomainAddr {
        SocketDomainAddr::new(Address::Domain(addr.into()), port)
    }

    pub fn new_ip<T: Into<IpAddr>>(addr: T, port: u16) -> SocketDomainAddr {
        SocketDomainAddr::new(Address::Ip(addr.into()), port)
    }
}

impl fmt::Display for SocketDomainAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.addr, self.port)
    }
}

impl From<SocketAddr> for SocketDomainAddr {
    fn from(addr: SocketAddr) -> Self {
        Self::new_ip(addr.ip(), addr.port())
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all(deserialize = "snake_case"))]
pub enum TransportType {
    Tcp,
    Udp,
}
