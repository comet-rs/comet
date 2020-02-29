use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use std::net::{Ipv4Addr, Ipv6Addr};

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub enum TcpState {
    SynSent,
    SynRcvd,
    Established,
    FinWait1,
    FinWait2,
    Closing,
    TimeWait,
    CloseWait,
    LastAck,
    Closed
}

pub struct TcpV4NatEntry {
    state: TcpState,
    last_activity: Instant,
    dest_addr: Ipv4Addr,
    dest_port: u16
}

pub struct TcpV6NatEntry {
    state: TcpState,
    last_activity: Instant,
    dest_addr: Ipv6Addr,
    dest_port: u16
}

pub struct UdpV4NatEntry {
    last_activity: Instant,
    dest_addr: Ipv4Addr,
    dest_port: u16
}

pub struct UdpV6NatEntry {
    last_activity: Instant,
    dest_addr: Ipv6Addr,
    dest_port: u16
}

pub struct NatManager {
    tcp_v4_table: BTreeMap<u16, TcpV4NatEntry>,
    tcp_v6_table: BTreeMap<u16, TcpV6NatEntry>,
    udp_v4_table: BTreeMap<u16, UdpV4NatEntry>,
    udp_v6_table: BTreeMap<u16, UdpV6NatEntry>
}

impl NatManager {
    pub fn new() -> Self {
        NatManager {
            tcp_v4_table: BTreeMap::new(),
            udp_v4_table: BTreeMap::new(),
            tcp_v6_table: BTreeMap::new(),
            udp_v6_table: BTreeMap::new()
        }
    }
}