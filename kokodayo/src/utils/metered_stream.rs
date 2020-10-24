use crate::app::metrics::MetricsValues;
use crate::prelude::*;
use futures::ready;
use futures::task::Context;
use futures::task::Poll;
use pin_project_lite::pin_project;
use std::io;
use std::pin::Pin;
use tokio::io::ReadBuf;

pin_project! {
  pub struct MeteredReader<R> {
    #[pin]
    inner: R,
    values: Arc<MetricsValues>
  }
}

impl<R: AsyncRead> MeteredReader<R> {
  pub fn new_inbound(inner: R, tag: &str, ctx: &AppContextRef) -> Self {
    let values = ctx.metrics.get_inbound(tag).unwrap();
    MeteredReader { inner, values }
  }

  pub fn new_outbound(inner: R, tag: &str, ctx: &AppContextRef) -> Self {
    let values = ctx.metrics.get_outbound(tag).unwrap();
    MeteredReader { inner, values }
  }
}

impl<R: AsyncRead> AsyncRead for MeteredReader<R> {
  fn poll_read(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &mut ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    let values = self.values.clone();
    let me = self.project();

    let filled_before = buf.filled().len();
    ready!(me.inner.poll_read(cx, buf))?;
    let filled_after = buf.filled().len();

    values.add_rx(filled_after - filled_before);

    Poll::Ready(Ok(()))
  }
}

pin_project! {
  pub struct MeteredWriter<W> {
    #[pin]
    inner: W,
    values: Arc<MetricsValues>
  }
}

impl<W: AsyncWrite> MeteredWriter<W> {
  pub fn new_inbound(inner: W, tag: &str, ctx: &AppContextRef) -> Self {
    let values = ctx.metrics.get_inbound(tag).unwrap();
    MeteredWriter { inner, values }
  }

  pub fn new_outbound(inner: W, tag: &str, ctx: &AppContextRef) -> Self {
    let values = ctx.metrics.get_outbound(tag).unwrap();
    MeteredWriter { inner, values }
  }
}

impl<W: AsyncWrite> AsyncWrite for MeteredWriter<W> {
  fn poll_write(
    self: Pin<&mut Self>,
    cx: &mut Context<'_>,
    buf: &[u8],
  ) -> Poll<io::Result<usize>> {
    let values = self.values.clone();
    let me = self.project();
    let r = ready!(me.inner.poll_write(cx, buf))?;
    values.add_tx(r);
    Poll::Ready(Ok(r))
  }

  fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    let me = self.project();
    me.inner.poll_flush(cx)
  }

  fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    let me = self.project();
    me.inner.poll_shutdown(cx)
  }
}

pin_project! {
  pub struct MeteredStream<RW> {
    #[pin]
    inner: MeteredReader<MeteredWriter<RW>>
  }
}
