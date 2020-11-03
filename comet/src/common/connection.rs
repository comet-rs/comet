use crate::TransportType;
use anyhow::{anyhow, Result};
use smol_str::SmolStr;
use std::any::Any;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;

#[derive(Debug, Default, Clone)]
pub struct DestAddr {
    pub domain: Option<SmolStr>,
    pub ip: Option<IpAddr>,
    pub port: Option<u16>,
}

impl DestAddr {
    pub fn set_domain<T: Into<SmolStr>>(&mut self, domain: T) {
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
}

#[derive(Debug)]
pub struct Connection {
    pub inbound_tag: SmolStr,
    pub inbound_pipeline: SmolStr,
    pub src_addr: SocketAddr,
    pub dest_addr: DestAddr,
    pub variables: HashMap<SmolStr, Box<dyn Any + Send + Sync>>,
    pub typ: TransportType,
    pub internal: bool,
}

impl Connection {
    pub fn new<A: Into<SocketAddr>, T1: Into<SmolStr>, T2: Into<SmolStr>>(
        src_addr: A,
        inbound_tag: T1,
        inbound_pipeline: T2,
        typ: TransportType,
    ) -> Self {
        Connection {
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

#[derive(Debug)]
pub struct UdpRequest {
    pub socket: Arc<UdpSocket>,
    pub packet: Vec<u8>,
}

impl UdpRequest {
    pub fn new(socket: Arc<UdpSocket>, packet: Vec<u8>) -> Self {
        Self { socket, packet }
    }
}
