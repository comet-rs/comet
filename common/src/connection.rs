use crate::Address;
use crate::RWPair;
use crate::SocketAddress;
use bytes::BytesMut;
use derivative::Derivative;
use std::net::SocketAddr;

#[derive(Debug)]
pub struct Connection {
    pub src_addr: SocketAddr,
    pub dest_addr: Option<SocketAddress>,
}

impl Connection {
    pub fn new<A: Into<SocketAddr>>(src_addr: A) -> Self {
        Connection {
            src_addr: src_addr.into(),
            dest_addr: None,
        }
    }
}

pub struct InboundConnection {
    pub conn: RWPair,
    pub addr: SocketAddr,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct AcceptedConnection {
    #[derivative(Debug = "ignore")]
    pub conn: RWPair,
    pub src_addr: SocketAddr,
    pub dest_addr: SocketAddress,

    #[derivative(Debug = "ignore")]
    pub sniffer_data: Option<BytesMut>,
    pub sniffed_dest: Option<Address>,
}

impl AcceptedConnection {
    pub fn new(conn: RWPair, src_addr: SocketAddr, dest_addr: SocketAddress) -> Self {
        AcceptedConnection {
            conn: conn,
            src_addr: src_addr,
            dest_addr: dest_addr,
            sniffer_data: None,
            sniffed_dest: None,
        }
    }
}

pub struct OutboundConnection {
    pub conn: RWPair,
}

impl OutboundConnection {
    pub fn new(conn: RWPair) -> Self {
        OutboundConnection { conn: conn }
    }
}
