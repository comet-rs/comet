use crate::prelude::*;
use anyhow::bail;

pub fn register(plumber: &mut Plumber) {
    plumber.register("socks5_server", |_, _| {
        Ok(Box::new(Socks5ProxyServerProcessor {}))
    });
}

mod v5 {
    pub const VERSION: u8 = 5;
    pub const METH_NO_AUTH: u8 = 0;
    pub const CMD_CONNECT: u8 = 1;
    pub const TYPE_IPV4: u8 = 1;
    pub const TYPE_IPV6: u8 = 4;
    pub const TYPE_DOMAIN: u8 = 3;
}

pub struct Socks5ProxyServerProcessor {}

#[async_trait]
impl Processor for Socks5ProxyServerProcessor {
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        conn: &mut Connection,
        _ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        let mut stream = stream.into_tcp()?;
        // Read version
        let version = stream.read_u8().await?;
        if version != v5::VERSION {
            bail!("Unsupported version: {}", version);
        }

        // Read and drop methods
        let nmethods = stream.read_u8().await?;
        stream.read_exact(&mut vec![0; nmethods as usize]).await?;

        // METHOD selection message
        stream.write_all(&[v5::VERSION, v5::METH_NO_AUTH]).await?;

        // Read request
        let addr_type = {
            let mut buffer = [0; 4]; // VER CMD RSV ATYP
            stream.read_exact(&mut buffer).await?;
            if buffer[1] != v5::CMD_CONNECT {
                bail!("Unsupported command: {}", buffer[1]);
            }
            buffer[3]
        };
        match addr_type {
            v5::TYPE_IPV4 => {
                let mut buffer = [0; 4];
                stream.read_exact(&mut buffer).await?;
                conn.dest_addr.set_ip(buffer);
            }
            v5::TYPE_IPV6 => {
                let mut buffer = [0; 16];
                stream.read_exact(&mut buffer).await?;
                conn.dest_addr.set_ip(buffer);
            }
            v5::TYPE_DOMAIN => {
                let mut buffer = [0; 255];
                let len = stream.read_u8().await? as usize;
                stream.read_exact(&mut buffer[0..len]).await?;
                let s = String::from_utf8_lossy(&buffer[0..len]);
                conn.dest_addr.set_domain(s);
            }
            _ => bail!("Invalid ATYP: {}", addr_type),
        }
        conn.dest_addr.set_port(stream.read_u16().await?);

        // Send reply
        stream
            .write_all(&[0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x08, 0x04])
            .await?;
        stream.flush().await?;

        // And we are done
        Ok(stream.into())
    }
}
