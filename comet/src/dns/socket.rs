use std::{
    marker::PhantomData,
    net::SocketAddr,
    sync::{RwLock, Weak},
    task::Context,
};

use futures::ready;
use once_cell::sync::{Lazy, OnceCell};
use tokio_stream::Stream;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};
use trust_dns_proto::{
    tcp::{Connect, DnsTcpStream},
    udp::UdpSocket,
    TokioTime,
};
use trust_dns_resolver::{
    name_server::{GenericConnection, GenericConnectionProvider, RuntimeProvider},
    AsyncResolver, TokioHandle,
};

use crate::{net_wrapper, prelude::*, utils::io::io_other_error};

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

    async fn bind(addr: SocketAddr) -> IoResult<Self> {
        let guard = CONTEXT.read().unwrap();
        let ctx = guard.upgrade().expect("App context dropped");
        let manager = ctx.clone_inbound_manager();
        let stream = manager
            .inject_udp("comet::dns", addr.ip().into())
            .map_err(io_other_error)?;

        Ok(Self {
            inner: RwLock::new(stream),
        })
    }

    fn poll_recv_from(
        &self,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<IoResult<(usize, SocketAddr)>> {
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
    ) -> Poll<IoResult<usize>> {
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

pub struct DirectUdpSocket(tokio::net::UdpSocket);

#[async_trait]
impl UdpSocket for DirectUdpSocket {
    type Time = TokioTime;

    async fn bind(addr: SocketAddr) -> IoResult<Self> {
        let socket = net_wrapper::bind_udp(&addr).await?;
        Ok(Self(socket))
    }

    fn poll_recv_from(
        &self,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<IoResult<(usize, SocketAddr)>> {
        UdpSocket::poll_recv_from(&self.0, cx, buf)
    }

    fn poll_send_to(
        &self,
        cx: &mut Context,
        buf: &[u8],
        target: SocketAddr,
    ) -> Poll<IoResult<usize>> {
        UdpSocket::poll_send_to(&self.0, cx, buf, target)
    }
}

impl DnsTcpStream for RWPair {
    type Time = TokioTime;
}

#[async_trait]
impl Connect for RWPair {
    async fn connect(addr: SocketAddr) -> IoResult<Self> {
        let guard = CONTEXT.read().unwrap();
        let ctx = guard.upgrade().expect("App context dropped");
        let manager = ctx.clone_inbound_manager();
        let stream = manager
            .inject_tcp("comet::dns", DestAddr::new_ip(addr.ip(), addr.port()))
            .map_err(io_other_error);

        Ok(stream?)
    }
}

pub struct DirectTcpStream(Compat<tokio::net::TcpStream>);

impl futures::AsyncRead for DirectTcpStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<IoResult<usize>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl futures::AsyncWrite for DirectTcpStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<IoResult<usize>> {
        Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.0).poll_close(cx)
    }
}

impl DnsTcpStream for DirectTcpStream {
    type Time = TokioTime;
}

#[async_trait]
impl Connect for DirectTcpStream {
    async fn connect(addr: SocketAddr) -> IoResult<Self> {
        let stream = net_wrapper::connect_tcp(&addr).await?;
        Ok(Self(stream.compat()))
    }
}

#[derive(Debug)]
pub struct CustomTokioRuntime<U, T>(PhantomData<U>, PhantomData<T>);

impl<U, T> Clone for CustomTokioRuntime<U, T> {
    fn clone(&self) -> Self {
        Self(PhantomData, PhantomData)
    }
}

impl<U, T> RuntimeProvider for CustomTokioRuntime<U, T>
where
    U: UdpSocket + Send + 'static,
    T: Connect + 'static,
{
    type Handle = TokioHandle;
    type Timer = TokioTime;
    type Tcp = T;
    type Udp = U;
}

pub type CustomTokioConnection = GenericConnection;
pub type CustomTokioConnectionProvider<U, T> = GenericConnectionProvider<CustomTokioRuntime<U, T>>;

pub type CustomTokioResolver =
    AsyncResolver<CustomTokioConnection, CustomTokioConnectionProvider<InternalUdpSocket, RWPair>>;

pub type CustomTokioResolverDirect = AsyncResolver<
    CustomTokioConnection,
    CustomTokioConnectionProvider<DirectUdpSocket, DirectTcpStream>,
>;
