use std::io;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{split, AsyncRead, AsyncWrite};

pub struct RWPair {
    pub read_half: Box<dyn AsyncRead + Unpin + Send + 'static>,
    pub write_half: Box<dyn AsyncWrite + Unpin + Send + 'static>,
}

impl RWPair {
    pub fn new<T: AsyncRead + AsyncWrite + Send + 'static>(inner: T) -> RWPair {
        let (read_half, write_half) = split(inner);
        RWPair {
            read_half: Box::new(read_half),
            write_half: Box::new(write_half),
        }
    }

    pub fn new_parts<R: AsyncRead + Send + Unpin + 'static, W: AsyncWrite + Send + Unpin + 'static>(
        read_half: R,
        write_half: W,
    ) -> RWPair {
        RWPair {
            read_half: Box::new(read_half),
            write_half: Box::new(write_half),
        }
    }
}

impl AsyncRead for RWPair {
    unsafe fn prepare_uninitialized_buffer(&self, buf: &mut [MaybeUninit<u8>]) -> bool {
        (*self.read_half).prepare_uninitialized_buffer(buf)
    }

    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut *self.get_mut().read_half).poll_read(cx, buf)
    }
}

impl AsyncWrite for RWPair {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut *self.get_mut().write_half).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.get_mut().write_half).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.get_mut().write_half).poll_shutdown(cx)
    }
}

macro_rules! impl_from {
    ($type:path) => {
        impl From<$type> for RWPair {
            fn from(s: $type) -> Self {
                Self::new(s)
            }
        }
    };
}

impl_from!(tokio::net::TcpStream);