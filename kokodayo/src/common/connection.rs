use crate::SocketDomainAddr;
use crate::TransportType;
use smol_str::SmolStr;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;

#[derive(Debug)]
pub struct Connection {
    pub inbound_tag: SmolStr,
    pub inbound_pipeline: SmolStr,
    pub src_addr: SocketAddr,
    pub dest_addr: Option<SocketDomainAddr>,
    pub variables: HashMap<SmolStr, SmolStr>,
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
            dest_addr: None,
            variables: HashMap::new(),
            typ,
            internal: false,
        }
    }

    pub fn set_var<K: Into<SmolStr>, V: Into<SmolStr>>(&mut self, key: K, value: V) {
        self.variables.insert(key.into(), value.into());
    }

    pub fn get_var(&self, key: &str) -> Option<&str> {
        self.variables.get(key).map(|v| v.borrow())
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
