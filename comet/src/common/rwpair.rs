use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::ReadBuf;
use tokio::io::{AsyncRead, AsyncWrite};

pub trait RWStream: AsyncRead + AsyncWrite + Send + Sync + Unpin {}

impl<T: AsyncRead + AsyncWrite + Send + Sync + Unpin> RWStream for T {}

pub enum RWPair {
    Full(Box<dyn RWStream + 'static>),
    Parts(
        Box<dyn AsyncRead + Unpin + Send + Sync + 'static>,
        Box<dyn AsyncWrite + Unpin + Send + Sync + 'static>,
    ),
}

impl RWPair {
    pub fn new<T: RWStream + 'static>(inner: T) -> RWPair {
        Self::Full(Box::new(inner))
    }

    pub fn new_parts<
        R: AsyncRead + Send + Unpin + Sync + 'static,
        W: AsyncWrite + Send + Unpin + Sync + 'static,
    >(
        read_half: R,
        write_half: W,
    ) -> RWPair {
        Self::Parts(Box::new(read_half), Box::new(write_half))
    }
}

impl AsyncRead for RWPair {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match &mut *self {
            RWPair::Full(ref mut inner) => Pin::new(inner).poll_read(cx, buf),
            RWPair::Parts(ref mut r, _) => Pin::new(r).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for RWPair {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut *self {
            RWPair::Full(ref mut inner) => Pin::new(inner).poll_write(cx, buf),
            RWPair::Parts(_, ref mut w) => Pin::new(w).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            RWPair::Full(ref mut inner) => Pin::new(inner).poll_flush(cx),
            RWPair::Parts(_, ref mut w) => Pin::new(w).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match &mut *self {
            RWPair::Full(ref mut inner) => Pin::new(inner).poll_shutdown(cx),
            RWPair::Parts(_, ref mut w) => Pin::new(w).poll_shutdown(cx),
        }
    }
}

impl std::fmt::Debug for RWPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "RWPair")
    }
}
