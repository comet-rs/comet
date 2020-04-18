use crate::IncomingStreamRawConnection;
use crate::IncomingStreamRawTransport;
use common::io::{StreamIO};
use async_trait::async_trait;
use futures::channel::mpsc::UnboundedSender;
use anyhow::Result;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use futures::SinkExt;

pub struct TcpIncomingStreamRawTransport {
    local_addr: SocketAddr,
}

#[async_trait]
impl IncomingStreamRawTransport for TcpIncomingStreamRawTransport {
    async fn start(&self, mut conn_sender: UnboundedSender<IncomingStreamRawConnection>) -> Result<()> {
        let mut listener = TcpListener::bind(self.local_addr).await?;
        loop {
            let (socket, src_addr) = listener.accept().await?;
            conn_sender.send((StreamIO::new(socket), src_addr)).await?;
        }
    }
}
