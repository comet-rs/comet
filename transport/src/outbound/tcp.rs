use crate::outbound::OutboundTransport;
use anyhow::Result;
use async_trait::async_trait;
use common::RWPair;
use net2::TcpBuilder;
use std::net::SocketAddr;
use tokio::net::TcpStream;

pub struct OutboundTcpTransport;

#[cfg(target_os = "android")]
async fn protect(unconnected: &std::net::TcpStream) -> io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let fd = unconnected.as_raw_fd();
    Ok(())
}

#[async_trait]
impl OutboundTransport for OutboundTcpTransport {
    async fn connect(&self, addr: SocketAddr) -> Result<RWPair<'static>> {
        let builder = match addr {
            SocketAddr::V4(_) => TcpBuilder::new_v4(),
            SocketAddr::V6(_) => TcpBuilder::new_v6(),
        }?;
        let unconnected = builder.to_tcp_stream()?;

        #[cfg(target_os = "android")]
        protect(&unconnected).await?;

        let stream = TcpStream::connect_std(unconnected, &addr).await?;
        Ok(RWPair::new(stream))
    }
}
