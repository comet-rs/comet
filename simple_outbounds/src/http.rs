use anyhow::Result;
use async_trait::async_trait;
use bytes::BytesMut;
use common::connection::{AcceptedConnection, OutboundConnection};
use common::protocol::OutboundProtocol;
use common::RWPair;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct HttpOutbound;

#[async_trait]
impl OutboundProtocol for HttpOutbound {
  async fn connect<'a>(
    &self,
    conn: &mut AcceptedConnection<'_>,
    mut downlink: RWPair<'a>,
  ) -> Result<OutboundConnection<'a>> {
    let request = format!("CONNECT {} HTTP/1.1\r\n\r\n", conn.dest_addr);
    downlink.write(request.as_bytes()).await?;
    let mut buffer = BytesMut::with_capacity(1024);

    loop {
      let mut headers = [httparse::EMPTY_HEADER; 16];
      let mut res = httparse::Response::new(&mut headers);
      downlink.read_buf(&mut buffer).await?;

      match res.parse(&buffer[..])? {
        httparse::Status::Complete(len) => {
          conn.conn.write(&buffer[len..]).await?; // Write response data
          return Ok(OutboundConnection::new(RWPair::new(downlink)));
        }
        httparse::Status::Partial => {}
      }
    }
  }
}
