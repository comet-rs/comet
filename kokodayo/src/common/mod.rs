use serde::Deserialize;
use smol_str::SmolStr;
use std::fmt;
use std::net::IpAddr;
use std::net::SocketAddr;

mod connection;
mod packet;
mod rwpair;

pub use connection::{Connection, DestAddr, UdpRequest};
pub use packet::{AsyncPacketIO, PacketIO};
pub use rwpair::RWPair;

#[derive(Debug, Clone)]
pub enum AddrKind {
    Domain(SmolStr),
    Ip(IpAddr),
}

impl fmt::Display for AddrKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AddrKind::Domain(s) => s.fmt(f),
            AddrKind::Ip(a) => a.fmt(f),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SocketDomainAddr {
    pub addr: AddrKind,
    pub port: u16,
}

impl SocketDomainAddr {
    pub fn new(addr: AddrKind, port: u16) -> SocketDomainAddr {
        SocketDomainAddr { addr, port }
    }

    pub fn new_domain<T: Into<SmolStr>>(addr: T, port: u16) -> SocketDomainAddr {
        SocketDomainAddr::new(AddrKind::Domain(addr.into()), port)
    }

    pub fn new_ip<T: Into<IpAddr>>(addr: T, port: u16) -> SocketDomainAddr {
        SocketDomainAddr::new(AddrKind::Ip(addr.into()), port)
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

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all(deserialize = "snake_case"))]
pub enum TransportType {
    Tcp,
    Udp,
}
