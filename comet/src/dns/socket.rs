use std::{
    net::SocketAddr,
    sync::{RwLock, Weak},
    task::Context,
};

use futures::ready;
use once_cell::sync::{Lazy, OnceCell};
use tokio_stream::Stream;
use trust_dns_proto::{udp::UdpSocket, TokioTime};

use crate::{prelude::*, utils::io::io_other_error};

static CONTEXT: Lazy<RwLock<Weak<AppContext>>> = Lazy::new(|| RwLock::new(Weak::new()));

pub fn init_ctx(ctx: AppContextRef) {
    let mut guard = CONTEXT.write().unwrap();
    let mut weak = Arc::downgrade(&ctx);
    std::mem::swap(&mut *guard, &mut weak);
}

/// Wrapper type for an internal UDP socket which is later injected
pub struct InternalUdpSocket {
    inner: RwLock<UdpStream>,
}

#[async_trait]
impl UdpSocket for InternalUdpSocket {
    type Time = TokioTime;

    async fn bind(_addr: SocketAddr) -> std::io::Result<Self> {
        let guard = CONTEXT.read().unwrap();
        let ctx = guard.upgrade().expect("App context dropped");
        let manager = ctx.clone_inbound_manager();
        let stream = manager.inject_udp("DNS").map_err(io_other_error)?;
        Ok(Self {
            inner: RwLock::new(stream),
        })
    }

    fn poll_recv_from(
        &self,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<(usize, SocketAddr)>> {
        let packet = {
            let mut guard = self.inner.write().unwrap();
            ready!(Pin::new(&mut *guard).poll_next(cx))
                .ok_or_else::<std::io::Error, _>(|| std::io::ErrorKind::BrokenPipe.into())?
        };

        let len = std::cmp::min(packet.len(), buf.len());
        buf[0..len].copy_from_slice(&packet[0..len]);
        // Excess bytes are discarded

        Poll::Ready(Ok((len, packet.target().unwrap())))
    }

    fn poll_send_to(
        &self,
        _cx: &mut Context,
        buf: &[u8],
        target: SocketAddr,
    ) -> Poll<std::io::Result<usize>> {
        let packet = UdpPacket::new(target, buf.into());
        let res = self
            .inner
            .read()
            .unwrap()
            .try_send(packet)
            .map(|_| buf.len())
            .map_err(io_other_error);
        Poll::Ready(res)
    }
}
