use crate::prelude::*;
use crate::utils::io::io_other_error;
use crate::utils::prepend_stream::PrependWriter;

pub fn register(plumber: &mut Plumber) {
    plumber.register("ss_handshake_client", |_| {
        Ok(Box::new(ShadowsocksClientHandshakeProcessor {}))
    });
}

#[derive(Debug)]
pub struct ShadowsocksClientHandshakeProcessor {}

impl ShadowsocksClientHandshakeProcessor {
    pub fn header_len(buf: &[u8]) -> IoResult<usize> {
        if buf.len() < 4 {
            return Err(io_other_error("header incomplete"));
        }
        let expected_len = match buf[0] {
            3 => 2 + buf[1] + 2,
            1 => 1 + 4 + 2,
            4 => 1 + 16 + 2,
            _ => return Err(io_other_error("invalid addr type")),
        } as usize;
        if buf.len() < expected_len {
            return Err(io_other_error("header incomplete"));
        }
        Ok(expected_len)
    }
}

#[async_trait]
impl Processor for ShadowsocksClientHandshakeProcessor {
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        conn: &mut Connection,
        _ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        let stream = stream.into_tcp()?;
        let dest_addr = &conn.dest_addr;
        let mut buf = if let Some(domain) = &dest_addr.domain {
            let mut buf = BytesMut::with_capacity(2 + domain.len() + 1);
            buf.put_u8(3);
            buf.put_u8(domain.len() as u8);
            buf.put_slice(domain.as_str().as_ref());
            buf
        } else {
            use std::net::IpAddr::*;
            let ip = dest_addr.ip_or_error()?;
            match ip {
                V4(ip) => {
                    let mut buf = BytesMut::with_capacity(1 + 4 + 1);
                    buf.put_u8(1);
                    buf.put_slice(&ip.octets());
                    buf
                }
                V6(ip) => {
                    let mut buf = BytesMut::with_capacity(1 + 16 + 1);
                    buf.put_u8(4);
                    buf.put_slice(&ip.octets());
                    buf
                }
            }
        };
        buf.put_u16(dest_addr.port_or_error()?);

        Ok(RWPair::new(PrependWriter::new(stream, buf)).into())
    }
}
