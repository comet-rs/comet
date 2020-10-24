use flurry::{HashMap, HashMapRef};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use std::time::Instant;

pub type NatManagerRef = Arc<NatManager>;
type NatMapRef<'a, T> = HashMapRef<'a, u16, NatEntry<T>>;

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
    Closed,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProtocolType {
    Tcp,
    Udp,
}

#[derive(Debug)]
struct NatEntry<Addr> {
    pub last_activity: Instant,
    pub dest_addr: Addr,
    pub dest_port: u16,
}

pub struct NatManager {
    tcp_v4_table: HashMap<u16, NatEntry<Ipv4Addr>>,
    tcp_v6_table: HashMap<u16, NatEntry<Ipv6Addr>>,
    udp_v4_table: HashMap<u16, NatEntry<Ipv4Addr>>,
    udp_v6_table: HashMap<u16, NatEntry<Ipv6Addr>>,
}

impl NatManager {
    pub fn new() -> Self {
        NatManager {
            tcp_v4_table: HashMap::new(),
            udp_v4_table: HashMap::new(),
            tcp_v6_table: HashMap::new(),
            udp_v6_table: HashMap::new(),
        }
    }

    pub fn new_ref() -> NatManagerRef {
        Arc::new(Self::new())
    }

    fn get_table_v4(&self, protocol: ProtocolType) -> NatMapRef<'_, Ipv4Addr> {
        match protocol {
            ProtocolType::Tcp => self.tcp_v4_table.pin(),
            ProtocolType::Udp => self.udp_v4_table.pin(),
        }
    }

    fn get_table_v6(&self, protocol: ProtocolType) -> NatMapRef<'_, Ipv6Addr> {
        match protocol {
            ProtocolType::Tcp => self.tcp_v6_table.pin(),
            ProtocolType::Udp => self.udp_v6_table.pin(),
        }
    }

    pub fn new_entry(
        &self,
        protocol: ProtocolType,
        src_port: u16,
        dest_addr: IpAddr,
        dest_port: u16,
    ) {
        match dest_addr {
            IpAddr::V4(addr) => {
                let entry = NatEntry {
                    last_activity: Instant::now(),
                    dest_addr: addr,
                    dest_port,
                };
                self.get_table_v4(protocol).insert(src_port, entry);
            }
            IpAddr::V6(addr) => {
                let entry = NatEntry {
                    last_activity: Instant::now(),
                    dest_addr: addr,
                    dest_port,
                };
                self.get_table_v6(protocol).insert(src_port, entry);
            }
        };
    }

    pub fn refresh_entry(
        &self,
        protocol: ProtocolType,
        src_port: u16,
        dest_addr: IpAddr,
        dest_port: u16,
    ) -> bool {
        match dest_addr {
            IpAddr::V4(addr) => {
                if let Some(entry) = self.get_table_v4(protocol).get(&src_port) {
                    if entry.dest_addr == addr && entry.dest_port == dest_port {
                        // entry.last_activity = Instant::now();
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            IpAddr::V6(addr) => {
                if let Some(entry) = self.get_table_v6(protocol).get(&src_port) {
                    if entry.dest_addr == addr && entry.dest_port == dest_port {
                        // entry.last_activity = Instant::now();
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        }
    }

    pub fn get_entry(
        &self,
        protocol: ProtocolType,
        port: u16,
        addr: IpAddr,
    ) -> Option<(IpAddr, u16)> {
        match addr {
            IpAddr::V4(_) => {
                if let Some(entry) = self.get_table_v4(protocol).get(&port) {
                    Some((IpAddr::V4(entry.dest_addr), entry.dest_port))
                } else {
                    None
                }
            }
            IpAddr::V6(_) => {
                if let Some(entry) = self.get_table_v6(protocol).get(&port) {
                    Some((IpAddr::V6(entry.dest_addr), entry.dest_port))
                } else {
                    None
                }
            }
        }
    }
}
