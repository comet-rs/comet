use crate::prelude::*;
use crate::{delegate_flush, delegate_read, delegate_shutdown, delegate_write_all};
use bytes::BytesMut;
use futures::ready;
use std::cmp;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::ReadBuf;

pub struct PrependReader<R> {
  inner: R,
  prepend: Option<BytesMut>,
}

impl<R: AsyncRead> PrependReader<R> {
  pub fn new(inner: R, prepend: BytesMut) -> Self {
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

delegate_write_all!(PrependReader);

pub struct PrependWriter<W> {
  inner: W,
  prepend: Option<BytesMut>,
  len_before_concat: Option<usize>,
  written: usize,
}

impl<W: AsyncWrite> PrependWriter<W> {
  pub fn new(inner: W, prepend: BytesMut) -> Self {
    let prepend = if prepend.is_empty() {
      None
    } else {
      Some(prepend)
    };
    PrependWriter {
      inner,
      prepend: prepend,
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
  delegate_flush!();
  delegate_shutdown!();
}

delegate_read!(PrependWriter);
