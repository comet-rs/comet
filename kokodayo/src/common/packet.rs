use anyhow::Result;
use async_trait::async_trait;
use tokio::net::UdpSocket;

#[async_trait]
pub trait AsyncPacketIO: Send + Sync + Unpin {
    async fn send(&mut self, buf: &[u8]) -> Result<usize>;
    async fn recv(&mut self, buf: &mut [u8]) -> Result<usize>;
}

pub struct PacketIO {
    inner: Box<dyn AsyncPacketIO + 'static>,
}

impl PacketIO {
    pub fn new<T: AsyncPacketIO + 'static>(inner: T) -> PacketIO {
        PacketIO {
            inner: Box::new(inner),
        }
    }
}

#[async_trait]
impl AsyncPacketIO for PacketIO {
    async fn send(&mut self, buf: &[u8]) -> Result<usize> {
        self.inner.send(buf).await
    }

    async fn recv(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.inner.recv(buf).await
    }
}

#[async_trait]
impl AsyncPacketIO for UdpSocket {
    async fn send(&mut self, buf: &[u8]) -> Result<usize> {
        self.send(buf).await
    }

    async fn recv(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.recv(buf).await
    }
}
