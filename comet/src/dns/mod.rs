use crate::config::Config;
use crate::crypto::rand::xor_rng;
use crate::net_wrapper::bind_udp;
use crate::prelude::*;
use anyhow::anyhow;
use lru_cache::LruCache;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use xorshift::Rng;

use trust_dns_proto::op::{Message, MessageType, OpCode, Query};
use trust_dns_proto::rr::{Name, Record, RecordType};
use trust_dns_proto::serialize::binary::BinEncodable;

const MAX_PAYLOAD_LEN: u16 = 1500 - 40 - 8;

fn new_lookup(query: &Query) -> Message {
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
    fake_map: Option<RwLock<LruCache<u16, SmolStr>>>,
}

impl DnsService {
    pub fn new(_config: &Config) -> Self {
        Self {
            cache: HashMap::new(),
            fake_map: Some(RwLock::new(LruCache::new(512))),
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

    pub async fn resolve_addr(&self, addr: &DestAddr) -> Result<Vec<IpAddr>> {
        if let Some(ip) = addr.ip {
            Ok(vec![ip])
        } else {
            let domain = addr.domain_or_error()?;
            Ok(tokio::net::lookup_host((domain, 443))
                .await?
                .map(|a| a.ip())
                .collect::<Vec<_>>())
        }
    }

    /// Blindly insert an item into the map
    pub async fn fake_set(&mut self, domain: &str) -> Ipv4Addr {
        let map_ref = self.fake_map.as_ref().unwrap();
        let mut map_ref_write = map_ref.write().await;
        let mut rng = xor_rng();
        // This is not optimal, but probably faster than iterating again.
        loop {
            let id: u16 = rng.gen();
            if !map_ref_write.contains_key(&id) {
                map_ref_write.insert(id, SmolStr::from(domain));
                let bytes = id.to_be_bytes();
                let ip = Ipv4Addr::new(10, 233, bytes[0], bytes[1]);
                break ip;
            }
        }
    }

    pub async fn fake_get(&mut self, addr: &Ipv4Addr) -> Option<SmolStr> {
        let map_ref = self.fake_map.as_ref().unwrap();
        let mut map_ref_write = map_ref.write().await;

        let octets = addr.octets();
        let id = u16::from_be_bytes([octets[2], octets[3]]);

        map_ref_write.get_mut(&id).map(|domain| domain.clone())
    }
}
