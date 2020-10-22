use anyhow::{anyhow, Result};
use bytes::{Buf, BytesMut};
use common::*;
use serde::Deserialize;
use tokio::prelude::*;

#[derive(Clone, Debug, Deserialize)]
pub struct HttpProxyClientConfig;
pub async fn client_handshake(mut stream: RWPair, conn: &mut Connection) -> Result<RWPair> {
  let request = format!(
    "CONNECT {0} HTTP/1.1\r\nHost: {0}\r\n\r\n",
    conn.dest_addr.as_ref().unwrap()
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
        return Ok(stream.prepend_data(buffer));
      }
      httparse::Status::Partial => {
        if n == 0 {
          return Err(anyhow!("Handshake failed: unexpected EOF"));
        }
      }
    }
  }
}
