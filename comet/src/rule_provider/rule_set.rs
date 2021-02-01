use std::{collections::HashSet, convert::TryFrom};

use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use regex::Regex;

use crate::{
    prelude::*,
    protos::v2ray::config::{GeoIP, GeoSite},
};

#[derive(Debug, Clone)]
pub enum RuleSet {
    Domain {
        full_domains: HashSet<SmolStr>,
        keywords: Vec<SmolStr>,
        regexes: Vec<Regex>,
        domains: Box<AhoCorasick>,
    },
    Ip,
}

impl RuleSet {
    pub fn is_match(&self, dest_addr: DestAddr) -> bool {
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
                let rev = to_reversed_fqdn(domain);
                domains.is_match(&rev)
                    || full_domains.contains(domain)
                    || regexes.iter().any(|re| re.is_match(domain))
                    || keywords.iter().any(|kw| domain.contains(kw.as_str()))
            }
            (RuleSet::Ip, _, Some(_ip)) => false,
            _ => false,
        }
    }
}

/// Converts `www.google.com` to `com.google.www.` for easier prefix matching
pub fn to_reversed_fqdn(domain: &str) -> String {
    // www.google.com => [com,google,www]
    let rev = domain.split('.').rev();
    // com.google.www. or cn.
    rev.chain(std::iter::once("")).collect::<Vec<_>>().join(".")
}

impl TryFrom<&GeoSite> for RuleSet {
    type Error = anyhow::Error;

    fn try_from(value: &GeoSite) -> Result<Self> {
        use crate::protos::v2ray::config::Domain_Type as DomainType;

        let mut full_domains = HashSet::new();
        let mut keywords = vec![];
        let mut domains = vec![];
        let mut regexes = vec![];

        for domain in &value.domain {
            match domain.field_type {
                DomainType::Plain => {
                    keywords.push(SmolStr::from(&domain.value));
                }
                DomainType::Regex => {
                    regexes.push(Regex::new(&domain.value)?);
                }
                DomainType::Domain => {
                    domains.push(to_reversed_fqdn(&domain.value));
                }
                DomainType::Full => {
                    full_domains.insert(SmolStr::from(&domain.value));
                }
            }
        }

        let ac = AhoCorasickBuilder::new()
            .auto_configure(&domains)
            .anchored(true)
            .build(&domains);

        let ret = Self::Domain {
            full_domains,
            keywords,
            regexes,
            domains: Box::new(ac),
        };

        Ok(ret)
    }
}

impl TryFrom<&GeoIP> for RuleSet {
    type Error = anyhow::Error;

    fn try_from(value: &GeoIP) -> Result<Self> {
        todo!()
    }
}
