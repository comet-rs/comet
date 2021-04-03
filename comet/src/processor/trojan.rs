use crate::{
    crypto::hashing::{hash_bytes, HashKind},
    prelude::*,
};
use tokio_prepend_io::PrependWriter;

pub fn register(plumber: &mut Plumber) {
    plumber.register("trojan_client", |conf, _| {
        let config: ClientConfig = from_value(conf)?;
        Ok(Box::new(ClientProcessor::new(config)))
    });
}

#[derive(Debug, Clone, Deserialize)]
struct ClientConfig {
    password: String,
}

struct ClientProcessor {
    password: String,
}

impl ClientProcessor {
    fn new(config: ClientConfig) -> Self {
        let hashed_password = hash_bytes(HashKind::Sha224, config.password.as_bytes());
        Self {
            password: hex::encode(hashed_password),
        }
    }
}

#[async_trait]
impl Processor for ClientProcessor {
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        conn: &mut Connection,
        _ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        let stream = stream.into_tcp()?;
        let dest_addr = &conn.dest_addr;
        let mut buf = BytesMut::with_capacity(56 + 2 + 4 + 2);
        /*
            +-----------------------+---------+----------------+---------+----------+
            | hex(SHA224(password)) |  CRLF   | Trojan Request |  CRLF   | Payload  |
            +-----------------------+---------+----------------+---------+----------+
            |          56           | X'0D0A' |    Variable    | X'0D0A' | Variable |
            +-----------------------+---------+----------------+---------+----------+

            where Trojan Request is a SOCKS5-like request:

            +-----+------+----------+----------+
            | CMD | ATYP | DST.ADDR | DST.PORT |
            +-----+------+----------+----------+
            |  1  |  1   | Variable |    2     |
            +-----+------+----------+----------+
            where:

            o  CMD
                o  CONNECT X'01'
                o  UDP ASSOCIATE X'03'
            o  ATYP address type of following address
                o  IP V4 address: X'01'
                o  DOMAINNAME: X'03'
                o  IP V6 address: X'04'
            o  DST.ADDR desired destination address
            o  DST.PORT desired destination port in network octet order
        */

        buf.extend_from_slice(self.password.as_bytes());
        buf.put_slice(&[0x0D, 0x0A, 0x01]);

        if let Some(domain) = &dest_addr.domain {
            buf.put_u8(3);
            buf.put_u8(domain.len() as u8);
            buf.put_slice(domain.as_str().as_ref());
        } else {
            use std::net::IpAddr::*;
            let ip = dest_addr.ip_or_error()?;
            match ip {
                V4(ip) => {
                    buf.put_u8(1);
                    buf.put_slice(&ip.octets());
                }
                V6(ip) => {
                    buf.put_u8(4);
                    buf.put_slice(&ip.octets());
                }
            }
        }
        buf.put_u16(dest_addr.port_or_error()?);
        buf.put_slice(&[0x0D, 0x0A]);

        Ok(RWPair::new(PrependWriter::new(stream, buf)).into())
    }
}
