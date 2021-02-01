use std::{borrow::Cow, net::IpAddr, time::Duration};

use anyhow::{anyhow, bail};
use trust_dns_resolver::{TokioHandle, config::{NameServerConfig, ResolverConfig, ResolverOpts}, system_conf::read_system_conf};
use url::Host;

use super::{socket::CustomTokioResolver, DnsConfigItem};
use crate::{prelude::*, router::matching::MatchCondition};

#[derive(Debug, Clone)]
pub struct Resolver {
    trust: CustomTokioResolver,
    rule: Option<MatchCondition>,
}

impl Resolver {
    pub fn from_config(item: &DnsConfigItem) -> Result<Self> {
        use trust_dns_resolver::config::Protocol;

        let mut resolver_opts = ResolverOpts::default();
        resolver_opts.timeout = item.timeout;
        resolver_opts.positive_min_ttl = Some(Duration::from_secs(300));
        resolver_opts.cache_size = if item.cache_size == 0 {
            128
        } else {
            item.cache_size
        };

        let mut name_servers = Vec::with_capacity(item.servers.len());
        for url in &item.servers {
            let ip: IpAddr = match url.host() {
                Some(Host::Ipv4(addr)) => addr.into(),
                Some(Host::Ipv6(addr)) => addr.into(),
                Some(Host::Domain(s)) => s
                    .parse()
                    .map_err(|_| anyhow!("DNS server must be an IP address, not {}", s))?,
                None => {
                    if url.scheme() == "system" {
                        let (sys_cfg, _) = read_system_conf()?;
                        name_servers.extend_from_slice(sys_cfg.name_servers());
                        continue;
                    }
                    bail!("Failed to parse DNS server address");
                }
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

            name_servers.push(NameServerConfig {
                socket_addr: (ip, port).into(),
                protocol,
                tls_dns_name: tls_name.map(|s| s.clone().into_owned()),
                trust_nx_responses: true,
                tls_config: None,
            });
        }

        if name_servers.is_empty() {
            bail!("No server in this resolver");
        }

        let trust = CustomTokioResolver::new(
            ResolverConfig::from_parts(None, vec![], name_servers),
            resolver_opts,
            TokioHandle,
        )?;

        Ok(Self {
            trust,
            rule: item.rule.clone(),
        })
    }

    pub fn from_system() -> Result<Self> {
        Ok(Self {
            trust: CustomTokioResolver::from_system_conf(TokioHandle)?,
            rule: None,
        })
    }

    pub async fn try_resolve(&self, domain: &str) -> Result<Option<Vec<IpAddr>>> {
      let result = self.trust.lookup_ip(domain).await?;
      let ans: Vec<IpAddr> = result.iter().collect();

      Ok(Some(ans))
    }
}
