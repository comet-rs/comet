use crate::prelude::*;
use anyhow::bail;
use futures::Future;
use hyper::{http::uri::Scheme, Uri};
use pin_project::pin_project;

#[derive(Clone)]
pub struct InternalHttpConnector {
    ctx: AppContextRef,
    tag: SmolStr,
}

impl InternalHttpConnector {
    pub fn new<T: Into<SmolStr>>(ctx: AppContextRef, tag: T) -> Self {
        Self {
            ctx,
            tag: tag.into(),
        }
    }

    async fn call_async(&mut self, dst: Uri) -> Result<RWPair> {
        let host = match dst.host() {
            Some(s) => s,
            None => bail!("URL Host is missing"),
        };
        let port = match dst.port() {
            Some(p) => p.as_u16(),
            None => {
                if dst.scheme() == Some(&Scheme::HTTPS) {
                    443
                } else {
                    80
                }
            }
        };

        let mut addr = DestAddr::default();
        addr.set_host_from_str(host);
        addr.set_port(port);

        self.ctx.inbound_manager.inject_tcp(&self.tag, addr)
    }
}

impl hyper::service::Service<Uri> for InternalHttpConnector {
    type Response = RWPair;
    type Error = anyhow::Error;
    type Future = ConnectingFut;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Uri) -> Self::Future {
        let mut this = self.clone();
        let task = async move { this.call_async(req).await };
        ConnectingFut(Box::pin(task))
    }
}

#[pin_project]
pub struct ConnectingFut(#[pin] Pin<Box<dyn Future<Output = Result<RWPair>> + Send>>);

impl Future for ConnectingFut {
    type Output = Result<RWPair>;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        self.project().0.poll(cx)
    }
}
