#![allow(unused_imports)]
use crate::net_wrapper::bind_udp;
use crate::prelude::*;
use crate::{config::Config, processor::tls_mitm};
use crate::{crypto::random::xor_rng, router::matching::MatchCondition};
use anyhow::anyhow;
use lru_cache::LruCache;
use rand::Rng;
use socket::{CustomTokioResolver, CustomTokioRuntime};
use std::{borrow::Cow, net::{IpAddr, Ipv4Addr, SocketAddr}, task::Context, time::Duration};
use std::{str::FromStr, time::SystemTime};
use tokio::sync::{Mutex, RwLock};
use trust_dns_resolver::{
    config::{NameServerConfig, NameServerConfigGroup, ResolverConfig, ResolverOpts},
    system_conf::read_system_conf,
    AsyncResolver, TokioHandle,
};
use url::{Host, Url};

use anyhow::bail;
use serde_with::{serde_as, DurationSeconds};
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

use self::{resolver::Resolver, socket::InternalUdpSocket};

mod resolver;
mod socket;

#[serde_as]
#[derive(Debug, Clone, Deserialize)]
pub struct DnsConfigItem {
    #[serde(default)]
    cache_size: usize,
    servers: Vec<Url>,
    /// Only do resolution if this rule evaluates to `true`.
    rule: Option<MatchCondition>,
    #[serde(default = "default_timeout")]
    #[serde_as(as = "DurationSeconds<u64>")]
    timeout: Duration,
    #[serde(default)]
    /// Requests will not go through Comet's network stack, reducing
    /// latency.
    direct: bool
}

fn default_timeout() -> Duration {
    Duration::from_secs(10)
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DnsConfig {
    #[serde(default)]
    resolvers: Vec<DnsConfigItem>,
}

pub struct DnsService {
    fake_map: Option<RwLock<LruCache<u16, SmolStr>>>,
    resolvers: Vec<Resolver>,
}

impl DnsService {
    pub fn new(config: &Config) -> Result<Self> {
        use trust_dns_resolver::config::Protocol;

        let dns_config = &config.dns;

        let mut resolvers = dns_config
            .resolvers
            .iter()
            .map(Resolver::from_config)
            .collect::<Result<Vec<_>>>()?;

        if resolvers.is_empty() {
            resolvers.push(Resolver::from_system()?);
        }

        Ok(Self {
            fake_map: Some(RwLock::new(LruCache::new(512))),
            resolvers,
        })
    }

    /// Initializes context for internal sockets
    pub fn start(&self, ctx: AppContextRef) {
        socket::init_ctx(ctx);
    }

    pub async fn resolve(&self, domain: &str) -> Result<Vec<IpAddr>> {
        for (i, res) in self.resolvers.iter().enumerate() {
            match res.try_resolve(domain).await {
                Ok(Some(result)) => {
                    debug!("Resolved {} -> {:?} with resolver #{}", domain, result, i);
                    return Ok(result);
                }
                Err(e) => {
                    return Err(e);
                }
                Ok(None) => {}
            }
        }

        Err(anyhow!("No resolver available for {}", domain))
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
