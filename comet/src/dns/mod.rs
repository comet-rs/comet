#![allow(unused_imports)]
use crate::crypto::rand::xor_rng;
use crate::net_wrapper::bind_udp;
use crate::prelude::*;
use crate::{config::Config, processor::tls_mitm};
use anyhow::anyhow;
use lru_cache::LruCache;
use socket::{CustomTokioResolver, CustomTokioRuntime};
use std::{
    borrow::Cow,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    task::Context,
};
use std::{str::FromStr, time::SystemTime};
use tokio::sync::{Mutex, RwLock};
use trust_dns_resolver::{
    config::{NameServerConfig, NameServerConfigGroup, ResolverConfig, ResolverOpts},
    AsyncResolver, TokioHandle,
};
use url::{Host, Url};
use xorshift::Rng;

use anyhow::bail;
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

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DnsConfig {
    #[serde(default)]
    cache_size: usize,
    resolvers: HashMap<SmolStr, Vec<Url>>,
}

pub struct DnsService {
    fake_map: Option<RwLock<LruCache<u16, SmolStr>>>,
    resolvers: HashMap<SmolStr, CustomTokioResolver>,
}

impl DnsService {
    pub fn new(config: &Config) -> Result<Self> {
        use trust_dns_resolver::config::Protocol;
        let dns_config = &config.dns;
        let mut resolver_opts = ResolverOpts::default();
        resolver_opts.cache_size = if dns_config.cache_size == 0 {
            128
        } else {
            dns_config.cache_size
        };

        let mut resolvers = dns_config
            .resolvers
            .iter()
            .map(|(tag, urls)| {
                let configs = urls
                    .iter()
                    .map(|url| {
                        let ip: IpAddr = match url.host() {
                            Some(Host::Ipv4(addr)) => addr.into(),
                            Some(Host::Ipv6(addr)) => addr.into(),
                            Some(Host::Domain(s)) => s.parse().map_err(|_| {
                                anyhow!("DNS server must be an IP address, not {}", s)
                            })?,
                            None => bail!("Failed to parse DNS server address"),
                        };

                        let protocol;
                        let port;
                        let params = url.query_pairs().collect::<HashMap<_, _>>();
                        let tls_name_default = Cow::Borrowed(url.host_str().unwrap());
                        let mut tls_name = params.get("domain").or(Some(&tls_name_default));

                        match url.scheme() {
                            "udp" => {
                                port = url.port().unwrap_or(53);
                                protocol = Protocol::Udp;
                                tls_name = None;
                            }
                            "https" => {
                                port = url.port().unwrap_or(443);
                                protocol = Protocol::Https;
                            }
                            "tls" => {
                                port = url.port().unwrap_or(853);
                                protocol = Protocol::Tls;
                            }
                            _ => bail!("Unknown scheme: {}", url.scheme()),
                        }

                        Ok(NameServerConfig {
                            socket_addr: (ip, port).into(),
                            protocol,
                            tls_dns_name: tls_name.map(|s| s.clone().into_owned()),
                            trust_nx_responses: true,
                            tls_config: None,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                let resolver = CustomTokioResolver::new(
                    ResolverConfig::from_parts(None, vec![], configs),
                    resolver_opts.clone(),
                    TokioHandle,
                )?;
                let ret = (tag.clone(), resolver);
                Ok(ret)
            })
            .collect::<Result<HashMap<_, _>>>()?;

        if !resolvers.contains_key("__SYSTEM") {
            resolvers.insert(
                "__SYSTEM".into(),
                CustomTokioResolver::from_system_conf(TokioHandle)?,
            );
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
        let client = self.resolvers.get("__SYSTEM").unwrap();
        let result = client.lookup_ip(domain).await?;

        let ans: Vec<IpAddr> = result.iter().collect();

        info!("Resolved {} -> {:?}", domain, ans);
        
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
