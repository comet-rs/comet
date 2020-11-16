use crate::prelude::*;
use crate::utils::io::eof;
use crate::utils::prepend_stream::PrependReader;
use anyhow::{anyhow, Result};
use bytes::{Buf, BytesMut};
use std::net::IpAddr;
use std::str::FromStr;

use httparse::{Request, Status};

pub fn register(plumber: &mut Plumber) {
  plumber.register("http_proxy_server", |_| Ok(Box::new(ServerProcessor {})));
}

pub struct ServerProcessor {}

#[async_trait]
impl Processor for ServerProcessor {
  async fn process(
    self: Arc<Self>,
    stream: ProxyStream,
    conn: &mut Connection,
    _ctx: AppContextRef,
  ) -> Result<ProxyStream> {
    let mut stream = stream.into_tcp()?;
    let mut buffer = BytesMut::with_capacity(512);
    loop {
      let mut headers = [httparse::EMPTY_HEADER; 32];
      let mut req = Request::new(&mut headers);
      if !buffer.has_remaining_mut() {
        buffer.reserve(512);
      }
      let n = stream.read_buf(&mut buffer).await?;

      match req.parse(&buffer[..])? {
        Status::Complete(len) => {
          for header in req.headers {
            if header.name.eq_ignore_ascii_case("Host") {
              let host = std::str::from_utf8(header.value)?;
              let mut split = host.split(':');
              let domain = split.next().unwrap();
              conn.dest_addr.port = Some(
                split
                  .next()
                  .and_then(|p| u16::from_str_radix(p, 10).ok())
                  .unwrap_or(80),
              );
              if let Ok(ip) = IpAddr::from_str(&domain) {
                conn.dest_addr.ip = Some(ip)
              } else {
                conn.dest_addr.domain = Some(domain.into());
              }
            }
          }

          if !conn.dest_addr.is_valid() {
            return Err(anyhow!("No or invalid Host header"));
          }

          let method = req.method.ok_or_else(|| anyhow!("No method specifed"))?;
          if method.eq_ignore_ascii_case("CONNECT") {
            // Strip header if doing CONNECTs
            buffer.advance(len);
          }
          let response = "HTTP/1.1 200 Connection Established\r\n\r\n";
          stream.write(response.as_bytes()).await?;
          return Ok(RWPair::new(PrependReader::new(stream, buffer)).into());
        }
        Status::Partial => {
          if n == 0 {
            return Err(eof().into());
          }
        }
      }
    }
  }
}
