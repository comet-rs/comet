use crate::prelude::*;

#[derive(Deserialize, Debug, Clone)]
pub struct ShadowsocksClientHandshakeConfig {}

pub struct ShadowsocksClientHandshakeProcessor {}

impl ShadowsocksClientHandshakeProcessor {
  pub fn new(_config: &ShadowsocksClientHandshakeConfig) -> Result<Self> {
    Ok(Self {})
  }
}

#[async_trait]
impl Processor for ShadowsocksClientHandshakeProcessor {
  async fn process(
    self: Arc<Self>,
    stream: RWPair,
    conn: &mut Connection,
    _ctx: AppContextRef,
  ) -> Result<RWPair> {
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

    Ok(stream.prepend_write(buf))
  }
}
