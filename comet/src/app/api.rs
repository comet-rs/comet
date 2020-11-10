use crate::prelude::*;
use http::{Request, Response};
use hyper::{server::conn::Http, service::service_fn, Body};
use tokio_compat_02::IoCompat;

pub async fn handle_api<S: AsyncRead + AsyncWrite + Unpin + 'static>(
  stream: S,
  ctx: AppContextRef,
) -> Result<()> {
  Ok(
    Http::new()
      .http1_only(true)
      .http1_keep_alive(true)
      .serve_connection(
        IoCompat::new(stream),
        service_fn(move |req| handle_req(req, ctx.clone())),
      )
      .await?,
  )
}

async fn handle_req(_req: Request<Body>, ctx: AppContextRef) -> Result<Response<Body>> {
  let froze = ctx.metrics.freeze();
  Ok(Response::new(Body::from(serde_json::to_vec(&froze)?)))
}
