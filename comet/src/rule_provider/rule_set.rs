use std::collections::HashSet;

use aho_corasick::AhoCorasick;
use regex::Regex;

use crate::prelude::*;

#[derive(Debug, Clone)]
pub enum RuleSet {
    Domain {
        full_domains: HashSet<SmolStr>,
        keywords: Vec<SmolStr>,
        regexes: Vec<Regex>,
        domains: AhoCorasick,
    },
    Ip,
}

impl RuleSet {
    pub fn is_match(&self, conn: &Connection) -> bool {
        match (self, &conn.dest_addr.domain, &conn.dest_addr.ip) {
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
                dbg!(&rev);
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