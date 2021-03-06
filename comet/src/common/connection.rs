use crate::TransportType;
use anyhow::{anyhow, Result};
use rand::{distributions::Alphanumeric, prelude::Distribution, thread_rng};
use serde_with::DeserializeFromStr;
use smol_str::SmolStr;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::fmt;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::{any::Any, str::FromStr};

#[derive(Debug, Default, Clone, DeserializeFromStr)]
pub struct DestAddr {
    pub domain: Option<SmolStr>,
    pub ip: Option<IpAddr>,
    pub port: Option<u16>,
}

impl DestAddr {
    pub fn new_domain<D: Into<SmolStr>>(domain: D, port: u16) -> Self {
        Self {
            domain: Some(domain.into()),
            ip: None,
            port: Some(port),
        }
    }

    pub fn new_ip<D: Into<IpAddr>>(ip: D, port: u16) -> Self {
        Self {
            domain: None,
            ip: Some(ip.into()),
            port: Some(port),
        }
    }

    pub fn set_domain<T: AsRef<str>>(&mut self, domain: T) {
        let domain = domain.as_ref().to_ascii_lowercase();
        self.domain = Some(domain.into());
    }

    pub fn set_ip<T: Into<IpAddr>>(&mut self, ip: T) {
        self.ip = Some(ip.into());
    }

    pub fn set_port(&mut self, port: u16) {
        self.port = Some(port);
    }

    pub fn ip_or_error(&self) -> Result<&IpAddr> {
        self.ip.as_ref().ok_or_else(|| anyhow!("Dest IP unknown"))
    }

    pub fn domain_or_error(&self) -> Result<&str> {
        self.domain
            .as_ref()
            .map(|d| d.borrow())
            .ok_or_else(|| anyhow!("Dest domain unknown"))
    }

    pub fn port_or_error(&self) -> Result<u16> {
        self.port.ok_or_else(|| anyhow!("Dest port unknown"))
    }

    pub fn is_valid(&self) -> bool {
        (self.domain.is_some() || self.ip.is_some()) && self.port.is_some()
    }

    pub fn set_host_from_str(&mut self, host: &str) {
        if let Ok(ip) = IpAddr::from_str(host) {
            self.ip = Some(ip);
        } else {
            self.domain = Some(host.into());
        }
    }
}

impl fmt::Display for DestAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        let domain = self.domain.as_ref().map(|s| s.as_str()).unwrap_or("?");
        let ip = self
            .ip
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "?".to_string());
        write!(f, "[{}/{}]:{}", domain, ip, self.port.unwrap_or(0))
    }
}

impl FromStr for DestAddr {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let mut this = Self::default();

        let mut split = s.splitn(2, ':');
        let host = split.next().unwrap();
        this.port = split.next().map(|port_s| port_s.parse()).transpose()?;

        this.set_host_from_str(host);

        Ok(this)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddrType {
    V4,
    V6,
}

impl From<IpAddr> for AddrType {
    fn from(a: IpAddr) -> Self {
        match a {
            IpAddr::V4(_) => Self::V4,
            IpAddr::V6(_) => Self::V6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionId([u8; 4]);

impl ConnectionId {
    pub fn new_rand() -> Self {
        let mut rng = thread_rng();
        let arr = [
            Alphanumeric.sample(&mut rng),
            Alphanumeric.sample(&mut rng),
            Alphanumeric.sample(&mut rng),
            Alphanumeric.sample(&mut rng),
        ];
        Self(arr)
    }
}

impl std::fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = unsafe { std::str::from_utf8_unchecked(&self.0) };
        f.write_str(s)
    }
}

#[derive(Debug)]
pub struct Connection {
    pub id: ConnectionId,
    pub inbound_tag: SmolStr,
    pub inbound_pipeline: Option<SmolStr>,
    pub src_addr: SocketAddr,
    pub dest_addr: DestAddr,
    pub variables: HashMap<SmolStr, Box<dyn Any + Send + Sync>>,
    pub typ: TransportType,
    pub internal: bool,
}

impl Connection {
    pub fn new<A: Into<SocketAddr>, T1: Into<SmolStr>, T2: Into<Option<SmolStr>>>(
        src_addr: A,
        inbound_tag: T1,
        inbound_pipeline: T2,
        typ: TransportType,
    ) -> Self {
        Connection {
            id: ConnectionId::new_rand(),
            inbound_tag: inbound_tag.into(),
            inbound_pipeline: inbound_pipeline.into(),
            src_addr: src_addr.into(),
            dest_addr: DestAddr::default(),
            variables: HashMap::new(),
            typ,
            internal: false,
        }
    }

    pub fn set_var<K: Into<SmolStr>, V: Any + Send + Sync>(&mut self, key: K, value: V) {
        self.variables.insert(key.into(), Box::new(value));
    }

    pub fn get_var<T: Any + Send + Sync>(&self, key: &str) -> Option<&T> {
        self.variables.get(key).and_then(|v| v.downcast_ref::<T>())
    }
}

impl fmt::Display for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(
            f,
            "({}) {}:{}@{} -> {}",
            self.id, self.typ, self.src_addr, self.inbound_tag, self.dest_addr
        )
    }
}

pub mod vars {
    pub static DEST: &str = "dest";
    pub static SS_KEY: &str = "ss-key";
    pub static SS_SALT: &str = "ss-salt";
}
