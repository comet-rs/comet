use crate::config::AndroidConfig;
use crate::config::Config;
use crate::prelude::*;
use crate::utils::unix_ts;
use flurry::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};

static TIMEOUT_TCP: u64 = 3600;
static TIMEOUT_UDP: u64 = 300;

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

#[derive(Debug)]
pub struct NatEntry<Addr> {
    pub last_activity: AtomicU64,
    pub dest_addr: Addr,
    pub dest_port: u16,
}

impl<T> NatEntry<T> {
    pub fn refresh(&self) {
        self.last_activity
            .store(unix_ts().as_secs(), Ordering::Relaxed);
    }
}

pub struct NatManager {
    pub config: AndroidConfig,
    tcp_v4_table: NatTable<Ipv4Addr>,
    tcp_v6_table: NatTable<Ipv6Addr>,
    udp_v4_table: NatTable<Ipv4Addr>,
    udp_v6_table: NatTable<Ipv6Addr>,
}

pub struct NatTable<Addr>(HashMap<u16, NatEntry<Addr>>);

impl<Addr: Send + Sync + 'static> NatTable<Addr> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn insert(&self, src_port: u16, dest_addr: Addr, dest_port: u16) {
        let entry = NatEntry {
            last_activity: AtomicU64::new(unix_ts().as_secs()),
            dest_addr: dest_addr,
            dest_port,
        };
        self.0.pin().insert(src_port, entry);
    }

    pub fn gc(&self, now: u64, timeout: u64) {
        let mut remove = vec![];
        let pinned = self.0.pin();
        for (src_port, entry) in pinned.iter() {
            if now - entry.last_activity.load(Ordering::Relaxed) > timeout {
                remove.push(src_port);
            }
        }
        for src_port in remove {
            pinned.remove(&src_port);
        }
    }
}

impl<Addr> Deref for NatTable<Addr> {
    type Target = HashMap<u16, NatEntry<Addr>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl NatManager {
    pub fn new(config: &Config) -> Self {
        NatManager {
            config: config.android.clone(),
            tcp_v4_table: NatTable::new(),
            udp_v4_table: NatTable::new(),
            tcp_v6_table: NatTable::new(),
            udp_v6_table: NatTable::new(),
        }
    }

    fn get_table_v4(&self, protocol: TransportType) -> &NatTable<Ipv4Addr> {
        match protocol {
            TransportType::Tcp => &self.tcp_v4_table,
            TransportType::Udp => &self.udp_v4_table,
        }
    }

    fn get_table_v6(&self, protocol: TransportType) -> &NatTable<Ipv6Addr> {
        match protocol {
            TransportType::Tcp => &self.tcp_v6_table,
            TransportType::Udp => &self.udp_v6_table,
        }
    }

    pub fn new_entry(
        &self,
        protocol: TransportType,
        src_port: u16,
        dest_addr: IpAddr,
        dest_port: u16,
    ) {
        match dest_addr {
            IpAddr::V4(addr) => {
                self.get_table_v4(protocol)
                    .insert(src_port, addr, dest_port);
            }
            IpAddr::V6(addr) => {
                self.get_table_v6(protocol)
                    .insert(src_port, addr, dest_port);
            }
        };
    }

    pub fn refresh_entry(
        &self,
        protocol: TransportType,
        src_port: u16,
        dest_addr: IpAddr,
        dest_port: u16,
    ) -> bool {
        match dest_addr {
            IpAddr::V4(addr) => {
                if let Some(entry) = self.get_table_v4(protocol).pin().get(&src_port) {
                    if entry.dest_addr == addr && entry.dest_port == dest_port {
                        entry.refresh();
                        return true;
                    }
                }
            }
            IpAddr::V6(addr) => {
                if let Some(entry) = self.get_table_v6(protocol).pin().get(&src_port) {
                    if entry.dest_addr == addr && entry.dest_port == dest_port {
                        entry.refresh();
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn get_entry(
        &self,
        protocol: TransportType,
        port: u16,
        addr: IpAddr,
    ) -> Option<(IpAddr, u16)> {
        match addr {
            IpAddr::V4(_) => {
                if let Some(entry) = self.get_table_v4(protocol).pin().get(&port) {
                    Some((IpAddr::V4(entry.dest_addr), entry.dest_port))
                } else {
                    None
                }
            }
            IpAddr::V6(_) => {
                if let Some(entry) = self.get_table_v6(protocol).pin().get(&port) {
                    Some((IpAddr::V6(entry.dest_addr), entry.dest_port))
                } else {
                    None
                }
            }
        }
    }

    pub fn gc(&self) {
        let now = unix_ts().as_secs();
        self.get_table_v4(TransportType::Tcp).gc(now, TIMEOUT_TCP);
        self.get_table_v6(TransportType::Tcp).gc(now, TIMEOUT_TCP);
        self.get_table_v4(TransportType::Udp).gc(now, TIMEOUT_UDP);
        self.get_table_v6(TransportType::Udp).gc(now, TIMEOUT_UDP);
    }
}
