use crate::Address;
use crate::RWPair;
use crate::SocketAddress;
use anyhow::Result;
use async_trait::async_trait;
use bytes::BytesMut;
use derivative::Derivative;
use std::net::SocketAddr;

pub struct InboundConnection<'conn> {
    pub conn: RWPair<'conn>,
    pub addr: SocketAddr,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct AcceptedConnection<'conn> {
    #[derivative(Debug = "ignore")]
    pub conn: RWPair<'conn>,
    
    pub src_addr: SocketAddr,
    pub dest_addr: SocketAddress,

    #[derivative(Debug = "ignore")]
    pub sniffer_data: Option<BytesMut>,
    pub sniffed_dest: Option<Address>,
}

impl<'conn> AcceptedConnection<'conn> {
    pub fn new(conn: RWPair<'conn>, src_addr: SocketAddr, dest_addr: SocketAddress) -> Self {
        AcceptedConnection {
            conn: conn,
            src_addr: src_addr,
            dest_addr: dest_addr,
            sniffer_data: None,
            sniffed_dest: None,
        }
    }
}

pub struct OutboundConnection<'conn> {
    pub conn: RWPair<'conn>,
}

impl<'conn> OutboundConnection<'conn> {
    pub fn new(conn: RWPair<'conn>) -> Self {
        OutboundConnection { conn: conn }
    }
}
