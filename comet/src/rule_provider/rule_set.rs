use std::{
    collections::HashSet,
    convert::{TryFrom, TryInto},
    net::Ipv4Addr,
};

use itertools::Itertools;
use regex::Regex;

use crate::{
    prelude::*,
    protos::v2ray::config::{GeoIP, GeoSite},
    router::matching::MatchMode,
};

#[derive(Debug)]
pub enum RuleSet {
    Domain {
        full_domains: HashSet<SmolStr>,
        keywords: Vec<SmolStr>,
        regexes: Vec<Regex>,
        domains: HashSet<SmolStr>,
    },
    Ip {
        v4: Ipv4List,
    },
}

impl RuleSet {
    pub fn is_match(&self, dest_addr: DestAddr, mode: MatchMode) -> bool {
        match (self, &dest_addr.domain, &dest_addr.ip) {
            (
                RuleSet::Domain {
                    full_domains,
                    keywords,
                    regexes,
                    domains,
                },
                Some(domain),
                _,
            ) => {
                mode.domain()
                    && (match_domain(domain, domains)
                        || full_domains.contains(domain)
                        || regexes.iter().any(|re| re.is_match(domain))
                        || keywords.iter().any(|kw| domain.contains(kw.as_str())))
            }
            (RuleSet::Ip { v4 }, _, Some(ip)) => {
                mode.ip()
                    && match ip {
                        std::net::IpAddr::V4(ip) => v4.is_match(*ip),
                        std::net::IpAddr::V6(_) => false,
                    }
            }
            _ => false,
        }
    }
}

/// Converts `www.google.com` to `com.google.www.` for easier prefix matching
pub fn to_reversed_fqdn(domain: &str) -> impl Iterator<Item = &str> {
    // www.google.com => [com,google,www]
    let rev = domain.split('.').rev();

    // com.google.www. or cn.
    let dots = std::iter::repeat_with(|| ".");
    rev.interleave_shortest(dots)
}

fn match_domain(domain: &str, set: &HashSet<SmolStr>) -> bool {
    let rev = to_reversed_fqdn(domain).collect_vec();

    for i in (2..rev.len()).step_by(2) {
        let s = rev[0..i].iter().copied().collect::<SmolStr>();

        if set.contains(&s) {
            return true;
        }
    }

    false
}

impl<'a> TryFrom<&GeoSite<'a>> for RuleSet {
    type Error = anyhow::Error;

    fn try_from(value: &GeoSite) -> Result<Self> {
        use crate::protos::v2ray::config::mod_Domain::Type as DomainType;

        let mut full_domains = HashSet::new();
        let mut keywords = vec![];
        let mut domains = HashSet::new();
        let mut regexes = vec![];

        for domain in &value.domain {
            match domain.type_pb {
                DomainType::Plain => {
                    keywords.push(SmolStr::from(domain.value.as_ref()));
                }
                DomainType::Regex => {
                    regexes.push(Regex::new(&domain.value)?);
                }
                DomainType::Domain => {
                    domains.insert(to_reversed_fqdn(&domain.value).collect());
                }
                DomainType::Full => {
                    full_domains.insert(SmolStr::from(domain.value.as_ref()));
                }
            }
        }

        let ret = Self::Domain {
            full_domains,
            keywords,
            regexes,
            domains,
        };

        Ok(ret)
    }
}

impl<'a> TryFrom<&GeoIP<'a>> for RuleSet {
    type Error = anyhow::Error;

    fn try_from(value: &GeoIP) -> Result<Self> {
        let mut list_v4 = Ipv4List::new();
        for cidr in &value.cidr {
            if cidr.ip.len() != 4 {
                continue;
            }

            let addr: [u8; 4] = cidr.ip[..].try_into().unwrap();
            let prefix = cidr.prefix as u8;
            list_v4.insert(addr, prefix);
        }

        Ok(Self::Ip { v4: list_v4 })
    }
}

#[derive(Debug)]
pub struct Ipv4List {
    map: HashMap<u8, Vec<(u8, [u8; 3])>>,
}

impl Ipv4List {
    fn new() -> Self {
        Self {
            map: HashMap::with_capacity(64),
        }
    }

    fn insert<A: Into<[u8; 4]>>(&mut self, addr: A, prefix: u8) {
        let addr: [u8; 4] = addr.into();

        let head = addr[0];
        let tail: [u8; 3] = addr[1..].try_into().unwrap();

        let item = (prefix, tail);
        let children = self.map.entry(head).or_insert_with(Vec::new);

        children.push(item);
    }

    fn is_match(&self, ip: Ipv4Addr) -> bool {
        let octets = ip.octets();
        let ip_tail = u32::from(ip) & 0x00FF_FFFF; // Exclude first octet

        self.map
            .get(&octets[0])
            .map(|children| {
                for (prefix, tail) in children {
                    let mask = !(0xffff_ffff_u64 >> *prefix) as u32;
                    let net = u32::from_be_bytes([0, tail[0], tail[1], tail[2]]) & mask;

                    if ip_tail & mask == net {
                        return true;
                    }
                }

                false
            })
            .unwrap_or(false)
    }
}
