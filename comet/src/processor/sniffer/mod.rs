mod http;
mod tls;
use crate::prelude::*;
use tokio_prepend_io::PrependReader;
use bytes::{BufMut, BytesMut};
use log::warn;
use std::net::IpAddr;
use std::str;
use std::str::FromStr;

pub fn register(plumber: &mut Plumber) {
    plumber.register("sniffer", |conf| {
        Ok(Box::new(SnifferProcessor {
            config: from_value(conf)?,
        }))
    });
}

#[derive(Debug)]
pub enum SniffStatus {
    NoClue,
    Fail(&'static str),
    Success(String),
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all(deserialize = "snake_case"))]
pub enum SniffType {
    Http,
    Tls,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SnifferConfig {
    #[serde(default)]
    types: Vec<SniffType>,
}

pub struct SnifferProcessor {
    config: SnifferConfig,
}

#[async_trait]
impl Processor for SnifferProcessor {
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        conn: &mut Connection,
        _ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        let mut stream = stream.into_tcp()?;
        let mut buffer = BytesMut::with_capacity(1024);

        let mut attempts: u8 = 0;
        let mut http_failed = !self.config.types.contains(&SniffType::Http);
        let mut tls_failed = !self.config.types.contains(&SniffType::Tls);

        while attempts < 5 && buffer.remaining_mut() > 0 {
            let read_bytes = stream.read_buf(&mut buffer).await?;
            if read_bytes == 0 {
                warn!("Got EOF while sniffing: {:?}", buffer);
                break;
            }

            if !http_failed {
                match http::sniff(&buffer) {
                    SniffStatus::NoClue => (),
                    SniffStatus::Fail(_) => {
                        http_failed = true;
                    }
                    SniffStatus::Success(s) => {
                        if let Some(idx) = s.rfind(':') {
                            s.split_at(idx);
                        }
                        conn.set_var("protocol", "http");
                        if let Ok(ip) = IpAddr::from_str(&s) {
                            conn.dest_addr.set_ip(ip);
                        } else {
                            conn.dest_addr.set_domain(s);
                        }
                        break;
                    }
                }
            }
            if !tls_failed {
                match tls::sniff(&buffer) {
                    SniffStatus::NoClue => (),
                    SniffStatus::Fail(_) => {
                        tls_failed = true;
                    }
                    SniffStatus::Success(s) => {
                        conn.set_var("protocol", "tls");
                        conn.dest_addr.set_domain(s);
                        break;
                    }
                }
            }
            if http_failed && tls_failed {
                break;
            }
            attempts += 1;
        }
        Ok(RWPair::new(PrependReader::new(stream, buffer)).into())
    }
}

impl Default for SnifferConfig {
    fn default() -> Self {
        SnifferConfig {
            types: vec![SniffType::Http, SniffType::Tls],
        }
    }
}
