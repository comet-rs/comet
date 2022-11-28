//! Wrapper types that prepends data when reading or writing with [`AsyncRead`] or [`AsyncWrite`].
use bytes::{Buf, BytesMut};
use futures::ready;
use std::cmp;
use std::io::Result;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::{delegate_read, delegate_write_all, delegate_write_misc};

/// A wrapper that produces prepended data before forwarding all read operations.
#[derive(Debug)]
pub struct PrependReader<R> {
    inner: R,
    prepend: Option<BytesMut>,
}

impl<R: AsyncRead> PrependReader<R> {
    /// Creates a new reader
    pub fn new<P: Into<BytesMut>>(inner: R, prepend: P) -> Self {
        let prepend = prepend.into();
        let prepend = if prepend.is_empty() {
            None
        } else {
            Some(prepend)
        };
        PrependReader { inner, prepend }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for PrependReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        if let Some(ref mut prepend_buf) = self.prepend {
            let n = cmp::min(buf.remaining(), prepend_buf.len());
            buf.put_slice(&prepend_buf[..n]);
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

delegate_write_all!(PrependReader);

/// A wrapper that writes prepended data to underlying stream before forwarding all writing operations.
#[derive(Debug)]
pub struct PrependWriter<W> {
    inner: W,
    prepend: Option<BytesMut>,
    len_before_concat: Option<usize>,
    written: usize,
}

impl<W: AsyncWrite> PrependWriter<W> {
    /// Creates a new writer
    pub fn new<P: Into<BytesMut>>(inner: W, prepend: P) -> Self {
        let prepend = prepend.into();
        let prepend = if prepend.is_empty() {
            None
        } else {
            Some(prepend)
        };
        PrependWriter {
            inner,
            prepend,
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
    ) -> Poll<Result<usize>> {
        let me = &mut *self;
        if let Some(ref mut prepend_buf) = me.prepend {
            // We need to write at least one byte in the actual input buffer
            loop {
                if me.len_before_concat.is_none() {
                    me.len_before_concat = Some(prepend_buf.len());
                    prepend_buf.extend_from_slice(buf); // Append our buffer with input data
                }

                let n = ready!(Pin::new(&mut me.inner).poll_write(cx, prepend_buf))?;
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
    delegate_write_misc!();
}

delegate_read!(PrependWriter);
