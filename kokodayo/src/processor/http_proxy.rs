use crate::prelude::*;
use anyhow::{anyhow, Result};
use bytes::{Buf, BytesMut};
use serde::Deserialize;
use tokio::prelude::*;

pub struct HttpProxyClientProcessor {}

impl HttpProxyClientProcessor {
  pub fn new(_config: &HttpProxyClientConfig) -> Result<Self> {
    Ok(HttpProxyClientProcessor {})
  }
}

#[derive(Clone, Debug, Deserialize)]
pub struct HttpProxyClientConfig {}

#[async_trait]
impl Processor for HttpProxyClientProcessor {
  async fn process(
    self: Arc<Self>,
    mut stream: RWPair,
    conn: &mut Connection,
    _ctx: AppContextRef,
  ) -> Result<RWPair> {
    let dest_addr = if let Some(domain) = &conn.dest_addr.domain {
      domain.to_string()
    } else {
      conn.dest_addr.ip_or_error()?.to_string()
    };
    let request = format!(
      "CONNECT {0}:{1} HTTP/1.1\r\nHost: {0}\r\n\r\n",
      dest_addr,
      conn.dest_addr.port_or_error()?
    );
    stream.write(request.as_bytes()).await?;
    let mut buffer = BytesMut::with_capacity(1024);
    loop {
      let mut headers = [httparse::EMPTY_HEADER; 16];
      let mut res = httparse::Response::new(&mut headers);
      let n = stream.read_buf(&mut buffer).await?;
      match res.parse(&buffer[..])? {
        httparse::Status::Complete(len) => {
          buffer.advance(len);
          return Ok(stream.prepend_read(buffer));
        }
        httparse::Status::Partial => {
          if n == 0 {
            return Err(anyhow!("Handshake failed: unexpected EOF"));
          }
        }
      }
    }
  }
}
