#![allow(unused_imports)]
use crate::config::Config;
use crate::crypto::rand::xor_rng;
use crate::net_wrapper::bind_udp;
use crate::prelude::*;
use anyhow::anyhow;
use lru_cache::LruCache;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    task::Context,
};
use std::{str::FromStr, time::SystemTime};
use tokio::sync::{Mutex, RwLock};
use trust_dns_client::{
    client::{AsyncClient, ClientHandle},
    rr::DNSClass,
};
use xorshift::Rng;

use trust_dns_proto::serialize::binary::BinEncodable;
use trust_dns_proto::{
    op::{Message, MessageType, OpCode, Query},
    udp::UdpSocket,
};
use trust_dns_proto::{
    rr::{Name, Record, RecordType},
    udp::UdpClientStream,
    TokioTime,
};

use self::socket::InternalUdpSocket;

mod socket;

const MAX_PAYLOAD_LEN: u16 = 1500 - 40 - 8;

fn new_lookup(query: &Query) -> Message {
    let mut message: Message = Message::new();
    let id: u16 = xor_rng().gen();

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

struct DnsEntry {
    time: SystemTime,
    result: Vec<IpAddr>,
}

impl DnsEntry {
    fn new(result: Vec<IpAddr>) -> Self {
        Self {
            time: SystemTime::now(),
            result,
        }
    }

    fn expired(&self) -> bool {
        if let Ok(elapsed) = self.time.elapsed() {
            elapsed.as_secs() > 3600
        } else {
            false
        }
    }

    fn clone_result(&self) -> Vec<IpAddr> {
        self.result.clone()
    }
}

pub struct DnsService {
    cache: flurry::HashMap<SmolStr, DnsEntry>,
    fake_map: Option<RwLock<LruCache<u16, SmolStr>>>,
    client: Mutex<Option<AsyncClient>>,
}

impl DnsService {
    pub fn new(_config: &Config) -> Self {
        Self {
            cache: flurry::HashMap::new(),
            fake_map: Some(RwLock::new(LruCache::new(512))),
            client: Mutex::new(None),
        }
    }

    /// Initializes DNS client tasks
    pub async fn start(&self, ctx: AppContextRef) -> Result<()> {
        socket::init_ctx(ctx);

        let addr = ([223, 6, 6, 6], 53).into();
        let stream = UdpClientStream::<InternalUdpSocket>::new(addr);
        let (client, bg) = AsyncClient::connect(stream).await?;
        tokio::spawn(bg);

        let mut guard = self.client.lock().await;
        *guard = Some(client);

        Ok(())
    }

    pub async fn process_query(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        let message = Message::from_vec(bytes)?;

        let (id, query) = Self::parse_query(&message)?;

        let upstream_query = new_lookup(&query);

        let mut upstream_response = xfer_message(upstream_query).await?;
        upstream_response.set_id(id);

        Ok(upstream_response.to_vec()?)
    }

    pub async fn resolve(&self, domain: &str) -> Result<Vec<IpAddr>> {
        {
            let cache_ref = self.cache.pin();
            if let Some(cached) = cache_ref.get(domain) {
                if !cached.expired() {
                    return Ok(cached.clone_result());
                }
            }
        }

        let result = {
            let mut guard = self.client.lock().await;
            let client = guard.as_mut().unwrap();
            client
                .query(Name::from_str(domain).unwrap(), DNSClass::IN, RecordType::A)
                .await?
        };

        let ans: Vec<IpAddr> = result
            .answers()
            .iter()
            .filter_map(|ans| ans.rdata().to_ip_addr())
            .collect();

        let cache_ref = self.cache.pin();
        cache_ref.insert(SmolStr::from(domain), DnsEntry::new(ans.clone()));

        debug!("Resolved {} -> {:?}", domain, ans);
        
        Ok(ans)
    }

    pub async fn resolve_addr(&self, addr: &DestAddr) -> Result<Vec<IpAddr>> {
        if let Some(ip) = addr.ip {
            Ok(vec![ip])
        } else {
            let domain = addr.domain_or_error()?;
            self.resolve(domain).await
        }
    }

    fn parse_query(message: &Message) -> Result<(u16, &Query)> {
        let id = message.id();
        let query = message
            .queries()
            .first()
            .ok_or_else(|| anyhow!("No query found in DNS request"))?;

        Ok((id, query))
    }

    pub async fn process_fake_dns(&self, input: &[u8]) -> Result<Vec<u8>> {
        let message = Message::from_vec(input)?;
        let (id, query) = Self::parse_query(&message)?;

        let upstream_query = new_lookup(&query);

        let mut upstream_response = xfer_message(upstream_query).await?;
        upstream_response.set_id(id);

        Ok(upstream_response.to_vec()?)
    }

    /// Blindly insert an item into the map
    pub async fn fake_set(&self, domain: &str) -> Ipv4Addr {
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

    pub async fn fake_get(&self, addr: &Ipv4Addr) -> Option<SmolStr> {
        let map_ref = self.fake_map.as_ref().unwrap();
        let mut map_ref_write = map_ref.write().await;

        let octets = addr.octets();
        let id = u16::from_be_bytes([octets[2], octets[3]]);

        map_ref_write.get_mut(&id).cloned()
    }
}
