use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait AsyncPacketRW: Send {
    async fn send(&mut self, buf: &[u8]) -> Result<usize>;
    async fn recv(&mut self, buf: &mut [u8]) -> Result<usize>;
}

pub struct PacketIO<'a> {
    inner: Box<dyn AsyncPacketRW + Unpin + Send + 'a>,
}

impl<'a> PacketIO<'a> {
    pub fn new<T: AsyncPacketRW + Send + Unpin + 'a>(inner: T) -> PacketIO<'a> {
        PacketIO {
            inner: Box::new(inner),
        }
    }
}

#[async_trait]
impl AsyncPacketRW for PacketIO<'_> {
    async fn send(&mut self, buf: &[u8]) -> Result<usize> {
        self.inner.send(buf).await
    }

    async fn recv(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.inner.recv(buf).await
    }
}
