#![allow(unused_imports)]
use crate::config::Config;
use crate::crypto::rand::xor_rng;
use crate::net_wrapper::bind_udp;
use crate::prelude::*;
use anyhow::anyhow;
use lru_cache::LruCache;
use socket::{CustomTokioResolver, CustomTokioRuntime};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    task::Context,
};
use std::{str::FromStr, time::SystemTime};
use tokio::sync::{Mutex, RwLock};
use trust_dns_resolver::{
    config::{NameServerConfigGroup, ResolverConfig, ResolverOpts},
    AsyncResolver, TokioHandle,
};
use xorshift::Rng;

use trust_dns_proto::{
    op::{Message, MessageType, OpCode, Query},
    udp::UdpSocket,
};
use trust_dns_proto::{rr::DNSClass, serialize::binary::BinEncodable};
use trust_dns_proto::{
    rr::{Name, Record, RecordType},
    udp::UdpClientStream,
    TokioTime,
};

use self::socket::InternalUdpSocket;

mod socket;

pub struct DnsService {
    fake_map: Option<RwLock<LruCache<u16, SmolStr>>>,
    resolvers: RwLock<HashMap<SmolStr, CustomTokioResolver>>,
}

impl DnsService {
    pub fn new(_config: &Config) -> Self {
        Self {
            fake_map: Some(RwLock::new(LruCache::new(512))),
            resolvers: RwLock::new(HashMap::new()),
        }
    }

    /// Initializes DNS client tasks
    pub async fn start(&self, ctx: AppContextRef) -> Result<()> {
        socket::init_ctx(ctx);

        let resolver_alidns = CustomTokioResolver::new(
            ResolverConfig::from_parts(
                None,
                vec![],
                NameServerConfigGroup::from_ips_https(
                    &[[223, 6, 6, 6].into(), [223, 5, 5, 5].into()],
                    443,
                    "dns.alidns.com".to_string(),
                    true,
                ),
            ),
            ResolverOpts::default(),
            TokioHandle,
        )?;

        let mut guard = self.resolvers.write().await;
        guard.insert(
            "__SYSTEM".into(),
            CustomTokioResolver::from_system_conf(TokioHandle)?,
        );
        guard.insert("alidns".into(), resolver_alidns);

        Ok(())
    }

    pub async fn resolve(&self, domain: &str) -> Result<Vec<IpAddr>> {
        let result = {
            let guard = self.resolvers.read().await;
            let client = guard.get("__SYSTEM").unwrap();
            client.lookup_ip(domain).await?
        };
        let ans: Vec<IpAddr> = result.iter().collect();
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

    pub fn parse_query(message: &Message) -> Result<(u16, &Query)> {
        let id = message.id();
        let query = message
            .queries()
            .first()
            .ok_or_else(|| anyhow!("No query found in DNS request"))?;

        Ok((id, query))
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
