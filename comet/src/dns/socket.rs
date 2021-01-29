use std::{net::SocketAddr, task::Context};

use once_cell::sync::OnceCell;
use trust_dns_proto::{udp::UdpSocket, TokioTime};

use crate::prelude::*;

struct InternalUdpSocket {
    ctx: AppContextRef,
    inner: UdpStream,
}

#[async_trait]
impl UdpSocket for InternalUdpSocket {
    type Time = TokioTime;

    async fn bind(addr: SocketAddr) -> std::io::Result<Self> {
        Ok(Self {
            inner: OnceCell::new(),
        })
    }

    fn poll_recv_from(
        &self,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<(usize, SocketAddr)>> {
        let mut buf = tokio::io::ReadBuf::new(buf);
        let addr = ready!(tokio::net::UdpSocket::poll_recv_from(self, cx, &mut buf))?;
        let len = buf.filled().len();

        Poll::Ready(Ok((len, addr)))
    }

    fn poll_send_to(
        &self,
        cx: &mut Context,
        buf: &[u8],
        target: SocketAddr,
    ) -> Poll<io::Result<usize>> {
        tokio::net::UdpSocket::poll_send_to(self, cx, buf, target)
    }
}
