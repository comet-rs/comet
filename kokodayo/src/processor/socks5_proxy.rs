use crate::prelude::*;
use anyhow::anyhow;
use std::net::IpAddr;

mod v5 {
  pub const VERSION: u8 = 5;
  pub const METH_NO_AUTH: u8 = 0;
  pub const CMD_CONNECT: u8 = 1;
  pub const TYPE_IPV4: u8 = 1;
  pub const TYPE_IPV6: u8 = 4;
  pub const TYPE_DOMAIN: u8 = 3;
}

pub struct Socks5ProxyServerProcessor {}

impl Socks5ProxyServerProcessor {
  pub fn new(_config: &Socks5ProxyServerConfig) -> Result<Self> {
    Ok(Socks5ProxyServerProcessor {})
  }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Socks5ProxyServerConfig {}

#[async_trait]
impl Processor for Socks5ProxyServerProcessor {
  async fn process(
    self: Arc<Self>,
    mut stream: RWPair,
    conn: &mut Connection,
    _ctx: AppContextRef,
  ) -> Result<RWPair> {
    // Read version
    let version = stream.read_u8().await?;
    if version != v5::VERSION {
      return Err(anyhow!("Unsupported version"));
    }

    // Read and drop methods
    let nmethods = stream.read_u8().await?;
    stream.read_exact(&mut vec![0; nmethods as usize]).await?;

    // METHOD selection message
    stream.write(&[v5::VERSION, v5::METH_NO_AUTH]).await?;

    // Read request
    let addr_type = {
      let mut buffer = [0; 4]; // VER CMD RSV ATYP
      stream.read_exact(&mut buffer).await?;
      if buffer[1] != v5::CMD_CONNECT {
        return Err(anyhow!("Unsupported command"));
      }
      buffer[3]
    };
    let address = match addr_type {
      v5::TYPE_IPV4 => {
        let mut buffer = [0; 4];
        stream.read_exact(&mut buffer).await?;
        Address::Ip(IpAddr::from(buffer))
      }
      v5::TYPE_IPV6 => {
        let mut buffer = [0; 16];
        stream.read_exact(&mut buffer).await?;
        Address::Ip(IpAddr::from(buffer))
      }
      v5::TYPE_DOMAIN => {
        let mut buffer = [0; 255];
        let len = stream.read_u8().await? as usize;
        stream.read_exact(&mut buffer[0..len]).await?;
        let s = String::from_utf8_lossy(&buffer[0..len]);
        Address::Domain(s.into())
      }
      _ => return Err(anyhow!("Invalid ATYP")),
    };
    let port = stream.read_u16().await?;
    conn.dest_addr = Some(SocketDomainAddr::new(address, port));

    // Send reply
    stream
      .write(&[0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x08, 0x4])
      .await?;

    // And we are done
    return Ok(stream);
  }
}
