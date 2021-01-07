use crate::prelude::*;
use anyhow::{anyhow, Result};
use bytes::BufMut;
use futures::ready;
use serde::Deserialize;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

mod connection;
mod context;
mod packet;
mod rwpair;

pub use connection::{Connection, DestAddr};
pub use context::{AppContext, AppContextRef};
pub use packet::UdpStream;
pub use rwpair::RWPair;

use std::sync::Arc;
use tokio::net::UdpSocket;

#[derive(Debug)]
pub enum ProxyStream {
    Tcp(RWPair),
    Udp(UdpStream),
}

impl ProxyStream {
    pub fn into_tcp(self) -> Result<RWPair> {
        match self {
            Self::Tcp(s) => Ok(s),
            Self::Udp(_) => Err(anyhow!("Incompatible type: UDP")),
        }
    }

    pub fn into_udp(self) -> Result<UdpStream> {
        match self {
            Self::Tcp(_) => Err(anyhow!("Incompatible type: TCP")),
            Self::Udp(s) => Ok(s),
        }
    }
}

impl From<RWPair> for ProxyStream {
    fn from(s: RWPair) -> Self {
        Self::Tcp(s)
    }
}
impl From<UdpStream> for ProxyStream {
    fn from(s: UdpStream) -> Self {
        Self::Udp(s)
    }
}

pub enum OutboundStream {
    Tcp(RWPair),
    Udp(Arc<UdpSocket>),
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all(deserialize = "snake_case"))]
pub enum TransportType {
    Tcp,
    Udp,
}

impl std::fmt::Display for TransportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        let s = match self {
            TransportType::Tcp => "TCP",
            TransportType::Udp => "UDP",
        };
        write!(f, "{}", s)
    }
}

pub trait MyAsyncReadExt: AsyncRead {
    /// Based on `read_buf`
    fn poll_read_buf<B: BufMut>(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut B,
    ) -> Poll<IoResult<usize>> {
        use std::mem::MaybeUninit;
        use tokio::io::ReadBuf;

        if !buf.has_remaining_mut() {
            return Poll::Ready(Ok(0));
        }

        let n = {
            let dst = buf.chunk_mut();
            let dst = unsafe { &mut *(dst as *mut _ as *mut [MaybeUninit<u8>]) };
            let mut buf = ReadBuf::uninit(dst);
            let ptr = buf.filled().as_ptr();
            ready!(self.poll_read(cx, &mut buf)?);

            // Ensure the pointer does not change from under us
            assert_eq!(ptr, buf.filled().as_ptr());
            buf.filled().len()
        };

        // Safety: This is guaranteed to be the number of initialized (and read)
        // bytes due to the invariants provided by `ReadBuf::filled`.
        unsafe {
            buf.advance_mut(n);
        }

        Poll::Ready(Ok(n))
    }
}

impl<T: AsyncRead> MyAsyncReadExt for T {}
