use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bytes::BytesMut;
use common::connection::{AcceptedConnection, OutboundConnection};
use common::protocol::OutboundProtocol;
use common::RWPair;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct HttpOutbound;

#[async_trait]
impl OutboundProtocol for HttpOutbound {
  async fn connect(
    &self,
    conn: &mut AcceptedConnection,
    mut downlink: RWPair,
  ) -> Result<OutboundConnection> {
    let request = format!("CONNECT {0} HTTP/1.1\r\nHost: {0}\r\n\r\n", conn.dest_addr);
    downlink.write(request.as_bytes()).await?;

    let mut buffer = BytesMut::with_capacity(1024);
    loop {
      let mut headers = [httparse::EMPTY_HEADER; 16];
      let mut res = httparse::Response::new(&mut headers);
      let n = downlink.read_buf(&mut buffer).await?;

      match res.parse(&buffer[..])? {
        httparse::Status::Complete(len) => {
          conn.conn.write(&buffer[len..]).await?; // Write response data
          return Ok(OutboundConnection::new(RWPair::new(downlink)));
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
