use crate::app::metrics::MetricsValues;
use crate::prelude::*;
use futures::ready;
use futures::task::Context;
use futures::task::Poll;
use pin_project::pin_project;
use std::pin::Pin;
use tokio::io::ReadBuf;

#[pin_project]
#[derive(Debug)]
pub struct MeteredStream<RW> {
    #[pin]
    inner: RW,
    conn_handle: Arc<()>,
    values: Arc<MetricsValues>,
}

impl<RW> MeteredStream<RW> {
    pub fn new_inbound(inner: RW, tag: &str, ctx: &AppContextRef) -> Self {
        let values = ctx.metrics.get_inbound(tag).unwrap();
        Self {
            inner,
            conn_handle: values.clone_conn_handle(),
            values,
        }
    }

    pub fn new_outbound(inner: RW, tag: &str, ctx: &AppContextRef) -> Self {
        let values = ctx.metrics.get_outbound(tag).unwrap();
        Self {
            inner,
            conn_handle: values.clone_conn_handle(),
            values,
        }
    }
}

impl<R: AsyncRead> AsyncRead for MeteredStream<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        let values = self.values.clone();
        let me = self.project();

        let filled_before = buf.filled().len();
        ready!(me.inner.poll_read(cx, buf))?;
        let filled_after = buf.filled().len();

        values.add_rx(filled_after - filled_before);

        Poll::Ready(Ok(()))
    }
}

impl<W: AsyncWrite> AsyncWrite for MeteredStream<W> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<IoResult<usize>> {
        let values = self.values.clone();
        let me = self.project();
        let r = ready!(me.inner.poll_write(cx, buf))?;
        values.add_tx(r);
        Poll::Ready(Ok(r))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        let me = self.project();
        me.inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        let me = self.project();
        me.inner.poll_shutdown(cx)
    }
}
