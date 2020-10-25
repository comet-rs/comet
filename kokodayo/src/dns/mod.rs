use crate::config::Config;
use crate::net_wrapper::bind_udp;
use crate::prelude::*;
use anyhow::anyhow;
use log::info;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;

use trust_dns_proto::op::{Message, MessageType, OpCode, Query};
use trust_dns_proto::rr::{Name, Record, RecordType};
use trust_dns_proto::serialize::binary::BinEncodable;

const MAX_PAYLOAD_LEN: u16 = 1500 - 40 - 8;

fn new_lookup(query: &Query) -> Message {
    info!("querying: {:?}", query);

    let mut message: Message = Message::new();
    let id: u16 = rand::random();

    message.add_query(query.clone());
    message
        .set_id(id)
        .set_message_type(MessageType::Query)
        .set_op_code(OpCode::Query)
        .set_recursion_desired(true);
    {
        let edns = message.edns_mut();
        edns.set_max_payload(MAX_PAYLOAD_LEN);
        edns.set_version(0);
    }

    message
}

async fn xfer_message(query: Message) -> Result<Message> {
    let message_raw = query.to_bytes()?;

    let out_sock = bind_udp(&SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0)).await?;
    out_sock.connect((Ipv4Addr::new(223, 6, 6, 6), 53)).await?;
    out_sock.send(&message_raw[..]).await?;

    let mut buffer = [0u8; 512];
    let size = out_sock.recv(&mut buffer[..]).await?;

    Ok(Message::from_vec(&buffer[0..size])?)
}

pub struct DnsService {
    cache: HashMap<Query, Record>,
}

impl DnsService {
    pub fn new(_config: &Config) -> Self {
        Self {
            cache: HashMap::new(),
        }
    }
    pub async fn process_query(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        let message_query = Message::from_vec(bytes)?;

        let id = message_query.id();
        let query = message_query
            .queries()
            .first()
            .ok_or_else(|| anyhow!("No query found in DNS request"))?;

        let upstream_query = new_lookup(&query);

        let mut upstream_response = xfer_message(upstream_query).await?;
        upstream_response.set_id(id);

        Ok(upstream_response.to_vec()?)
    }

    pub async fn resolve(&self, domain: &str) -> Result<()> {
        let query = Query::query(Name::from_str(domain)?, RecordType::A);
        Ok(())
    }

    pub async fn resolve_addr(&self, addr: &Address) -> Result<Vec<IpAddr>> {
        Ok(match addr {
            Address::Ip(ip) => vec![*ip],
            Address::Domain(s) => {
                let s: &str = s.borrow();
                tokio::net::lookup_host((s, 443))
                    .await?
                    .map(|a| a.ip())
                    .collect()
            }
        })
    }
}
