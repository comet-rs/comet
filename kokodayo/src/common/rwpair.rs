use bytes::{Buf, BytesMut};
use futures::{ready, try_join};
use std::cmp;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::ReadBuf;
use tokio::io::{copy, split, AsyncRead, AsyncWrite};

pub trait RWStream: AsyncRead + AsyncWrite + Send + Sync + Unpin {}

pub struct RWPair {
    pub read_half: Box<dyn AsyncRead + Unpin + Send + 'static>,
    pub write_half: Box<dyn AsyncWrite + Unpin + Send + 'static>,
}

impl RWPair {
    pub fn new<T: AsyncRead + AsyncWrite + Send + 'static>(inner: T) -> RWPair {
        let (read_half, write_half) = split(inner);
        Self::new_parts(read_half, write_half)
    }

    pub fn new_parts<
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
    >(
        read_half: R,
        write_half: W,
    ) -> RWPair {
        RWPair {
            read_half: Box::new(read_half),
            write_half: Box::new(write_half),
        }
    }

    /// Schedule extra data to be read before the inner stream.
    pub fn prepend_read<T: Into<BytesMut>>(mut self, data: T) -> Self {
        self.read_half = Box::new(PrependReader::new(self.read_half, data.into()));
        self
    }

    /// Schedule extra data to be written before any external data.
    pub fn prepend_write<T: Into<BytesMut>>(mut self, data: T) -> Self {
        self.write_half = Box::new(PrependWriter::new(self.write_half, data.into()));
        self
    }

    pub async fn bidi_copy(&mut self, other: &mut Self) -> io::Result<(u64, u64)> {
        let read = copy(&mut self.read_half, &mut other.write_half);
        let write = copy(&mut other.read_half, &mut self.write_half);
        Ok(try_join!(read, write)?)
    }
}

struct PrependReader<R> {
    inner: R,
    prepend: Option<BytesMut>,
}

impl<R: AsyncRead> PrependReader<R> {
    fn new(inner: R, prepend: BytesMut) -> Self {
        PrependReader {
            inner,
            prepend: Some(prepend),
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for PrependReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if let Some(ref mut prepend_buf) = self.prepend {
            let n = cmp::min(buf.remaining(), prepend_buf.len());
            buf.put_slice(&prepend_buf.bytes()[..n]);
            prepend_buf.advance(n);
            if prepend_buf.is_empty() {
                // All consumed
                self.prepend = None;
            }
            Poll::Ready(Ok(()))
        } else {
            Pin::new(&mut self.inner).poll_read(cx, buf)
        }
    }
}

struct PrependWriter<W> {
    inner: W,
    prepend: Option<BytesMut>,
    len_before_concat: Option<usize>,
    written: usize,
}

impl<W: AsyncWrite> PrependWriter<W> {
    fn new(inner: W, prepend: BytesMut) -> Self {
        PrependWriter {
            inner,
            prepend: Some(prepend),
            len_before_concat: None,
            written: 0,
        }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for PrependWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let me = &mut *self;
        if let Some(ref mut prepend_buf) = me.prepend {
            loop {
                if me.len_before_concat.is_none() {
                    me.len_before_concat = Some(prepend_buf.len());
                    prepend_buf.extend_from_slice(buf); // Append our buffer with input data
                }

                let n = ready!(Pin::new(&mut me.inner).poll_write(cx, &prepend_buf))?;
                me.written += n;

                if me.written > me.len_before_concat.unwrap() {
                    me.prepend = None;
                    return Poll::Ready(Ok(me.written - me.len_before_concat.unwrap()));
                }
            }
        } else {
            Pin::new(&mut self.inner).poll_write(cx, buf)
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl AsyncRead for RWPair {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.read_half).poll_read(cx, buf)
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

impl std::fmt::Debug for RWPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "RWPair")
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
