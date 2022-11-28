use crate::prelude::*;
use crate::utils::io::eof;
use crate::utils::prepend_io::PrependReader;
use bytes::{Buf, BytesMut};

pub fn register(plumber: &mut Plumber) {
    plumber.register("http_proxy_client", |_, _| Ok(Box::new(ClientProcessor {})));
}

pub struct ClientProcessor {}

#[async_trait]
impl Processor for ClientProcessor {
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        conn: &mut Connection,
        _ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        let mut stream = stream.into_tcp()?;
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
        stream.write_all(request.as_bytes()).await?;
        let mut buffer = BytesMut::with_capacity(512);
        loop {
            let mut headers = [httparse::EMPTY_HEADER; 16];
            let mut res = httparse::Response::new(&mut headers);
            if !buffer.has_remaining_mut() {
                buffer.reserve(512);
            }
            let n = stream.read_buf(&mut buffer).await?;
            match res.parse(&buffer[..])? {
                httparse::Status::Complete(len) => {
                    buffer.advance(len);
                    return Ok(RWPair::new(PrependReader::new(stream, buffer)).into());
                }
                httparse::Status::Partial => {
                    if n == 0 {
                        return Err(eof().into());
                    }
                }
            }
        }
    }
}
