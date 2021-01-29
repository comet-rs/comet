use crate::prelude::*;
use std::{net::SocketAddr, task::Context};
use tokio::sync::mpsc::Sender;
use tokio_stream::Stream;

#[derive(Debug, Clone)]
pub struct UdpPacket {
    target: Option<SocketAddr>,
    payload: BytesMut,
}

impl UdpPacket {
    pub fn new(target: SocketAddr, payload: BytesMut) -> Self {
        Self {
            target: Some(target),
            payload,
        }
    }

    pub fn new_unknown(payload: BytesMut) -> Self {
        Self {
            target: None,
            payload,
        }
    }

    pub fn target(&self) -> Option<SocketAddr> {
        return self.target;
    }
}

impl std::ops::Deref for UdpPacket {
    type Target = BytesMut;

    fn deref(&self) -> &Self::Target {
        &self.payload
    }
}

impl std::ops::DerefMut for UdpPacket {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.payload
    }
}

pub struct UdpStream {
    read: Box<dyn Stream<Item = UdpPacket> + Send + Sync + Unpin>,
    write: Sender<UdpPacket>,
}

impl std::fmt::Debug for UdpStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "UdpStream")
    }
}

impl UdpStream {
    pub fn new<T: Stream<Item = UdpPacket> + Send + Sync + Unpin + 'static>(
        read: T,
        write: Sender<UdpPacket>,
    ) -> Self {
        Self {
            read: Box::new(read),
            write,
        }
    }

    pub async fn send(&self, data: UdpPacket) -> Result<()> {
        Ok(self.write.send(data).await?)
    }
}

impl Stream for UdpStream {
    type Item = UdpPacket;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut *self.read).poll_next(cx)
    }
}
