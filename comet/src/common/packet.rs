use crate::prelude::*;
use std::task::Context;
use tokio::sync::mpsc::Sender;
use tokio_stream::Stream;

pub struct UdpStream {
    read: Box<dyn Stream<Item = BytesMut> + Send + Sync + Unpin>,
    write: Sender<BytesMut>,
}

impl std::fmt::Debug for UdpStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "UdpStream")
    }
}

impl UdpStream {
    pub fn new<T: Stream<Item = BytesMut> + Send + Sync + Unpin + 'static>(
        read: T,
        write: Sender<BytesMut>,
    ) -> Self {
        Self {
            read: Box::new(read),
            write,
        }
    }

    pub async fn send(&self, data: BytesMut) -> Result<()> {
        Ok(self.write.send(data).await?)
    }
}

impl Stream for UdpStream {
    type Item = BytesMut;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut *self.read).poll_next(cx)
    }
}
