mod http;
mod tls;
use crate::prelude::*;
use bytes::{BufMut, BytesMut};
use log::warn;
use std::str;

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
    #[serde(default)]
    override_dest: bool,
}

pub struct SnifferProcessor {
    config: SnifferConfig,
}

impl SnifferProcessor {
    pub fn new(config: &SnifferConfig) -> Result<Self> {
        Ok(SnifferProcessor {
            config: config.clone()
        })
    }
}

#[async_trait]
impl Processor for SnifferProcessor {
    async fn process(
        self: Arc<Self>,
        mut stream: RWPair,
        conn: &mut Connection,
        _ctx: AppContextRef,
    ) -> Result<RWPair> {
        let mut buffer = BytesMut::with_capacity(1024);

        let mut attempts: u8 = 0;
        let mut http_failed = !self.config.types.contains(&SniffType::Http);
        let mut tls_failed = !self.config.types.contains(&SniffType::Tls);

        let dest_port = conn.dest_addr.as_ref().unwrap().port;
        while attempts < 5 && buffer.remaining_mut() > 0 {
            let read_bytes = stream.read_buf(&mut buffer).await?;
            if read_bytes == 0 {
                warn!("Got EOF while sniffing: {:?}", buffer);
                return Ok(stream.prepend_data(buffer));
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
                        conn.set_var("sniffed_dest", &s);
                        conn.set_var("protocol", "http");
                        if self.config.override_dest {
                            conn.dest_addr = Some(SocketDomainAddr::new_domain(s, dest_port));
                        }
                        return Ok(stream.prepend_data(buffer));
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
                        conn.set_var("sniffed_dest", &s);
                        conn.set_var("protocol", "tls");
                        if self.config.override_dest {
                            conn.dest_addr = Some(SocketDomainAddr::new_domain(s, dest_port));
                        }
                        return Ok(stream.prepend_data(buffer));
                    }
                }
            }
            if http_failed && tls_failed {
                break;
            }
            attempts += 1;
        }
        return Ok(stream.prepend_data(buffer));
    }
}

impl Default for SnifferConfig {
    fn default() -> Self {
        SnifferConfig {
            types: vec![SniffType::Http, SniffType::Tls],
            override_dest: false,
        }
    }
}
