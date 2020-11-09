use crate::prelude::*;
use http::{Request, Response};
use hyper::{server::conn::Http, service::service_fn, Body};
use std::convert::Infallible;
use tokio_compat_02::IoCompat;

pub async fn handle_api<S: AsyncRead + AsyncWrite + Unpin + 'static>(
  stream: S,
  ctx: AppContextRef,
) -> Result<()> {
  Ok(
    Http::new()
      .http1_only(true)
      .http1_keep_alive(true)
      .serve_connection(IoCompat::new(stream), service_fn(hello))
      .await?,
  )
}

async fn hello(_req: Request<Body>) -> Result<Response<Body>, Infallible> {
  Ok(Response::new(Body::from("Hello World!")))
}
