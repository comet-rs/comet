use std::net::SocketAddr;
use common::io::{StreamIO, PacketIO};
use async_trait::async_trait;
use futures::channel::mpsc::{UnboundedSender, UnboundedReceiver, unbounded};
use anyhow::Result;

mod tcp;
mod manager;
pub use manager::{IncomingRawTransportManager, OutgoingRawTransportManager};

pub type IncomingStreamRawConnection = (StreamIO<'static>, SocketAddr);
pub type IncomingPacketRawConnection = (PacketIO<'static>, SocketAddr);

#[async_trait]
pub trait IncomingStreamRawTransport: Send {
    async fn start(&self, mut conn_sender: UnboundedSender<IncomingStreamRawConnection>) -> Result<()>;
}


#[async_trait]
pub trait IncomingPacketRawTransport: Send {
    async fn start(&self, mut conn_sender: UnboundedSender<IncomingPacketRawConnection>) -> Result<()>;
}