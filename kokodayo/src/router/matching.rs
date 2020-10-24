use crate::prelude::*;
use anyhow::{anyhow, Result};
use common::Connection;
use ipnetwork::IpNetwork;
use regex::RegexSet;
use serde::de::Error;
use serde::Deserializer;
use serde::{Serialize, Serializer};
use serde_with::DeserializeFromStr;
use std::borrow::Borrow;
use std::net::IpAddr;
use std::str::FromStr;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all(deserialize = "snake_case"))]
pub enum MatchCondition {
  Any(Vec<MatchCondition>),
  All(Vec<MatchCondition>),
  DestAddr(Vec<IpMatchCondition>),
  SrcAddr(Vec<IpMatchCondition>),
  DestDomain(#[serde(deserialize_with = "deserialize_domain_matcher_text")] DomainMatcher),
  Metadata,
}

impl MatchCondition {
  pub fn is_match(&self, conn: &Connection) -> bool {
    match self {
      MatchCondition::Any(conds) => {
        for cond in conds {
          if cond.is_match(conn) {
            return true;
          }
        }
        false
      }
      MatchCondition::All(conds) => {
        for cond in conds {
          if !cond.is_match(conn) {
            return false;
          }
        }
        true
      }
      MatchCondition::DestAddr(conds) => {
        if let Some(addr) = &conn.dest_addr {
          if let common::Address::Ip(ip) = &addr.addr {
            for cond in conds {
              if cond.is_match(ip) {
                return true;
              }
            }
          }
        }
        false
      }
      MatchCondition::SrcAddr(conds) => {
        let ip = conn.src_addr.ip();
        for cond in conds {
          if cond.is_match(&ip) {
            return true;
          }
        }
        false
      }
      MatchCondition::DestDomain(cond) => {
        if let Some(addr) = &conn.dest_addr {
          if let common::Address::Domain(domain) = &addr.addr {
            return cond.is_match(domain);
          }
        }
        false
      }
      MatchCondition::Metadata => false,
    }
  }
}

macro_rules! strip_index_variant {
  ($input:expr, $prefix:expr, $variant:expr) => {
    if let Some(res) = $input.strip_prefix($prefix) {
      return Ok($variant(res.into()));
    }
  };
}

#[derive(Debug, Clone, DeserializeFromStr)]
pub enum IpMatchCondition {
  Addr(IpAddr),
  Cidr(IpNetwork),
  GeoIp(String),
}

impl IpMatchCondition {
  pub fn is_match(&self, addr: &IpAddr) -> bool {
    match self {
      IpMatchCondition::Addr(addr_) => addr_ == addr,
      IpMatchCondition::Cidr(net) => net.contains(*addr),
      IpMatchCondition::GeoIp(_loc) => false,
    }
  }
}

impl FromStr for IpMatchCondition {
  type Err = anyhow::Error;

  fn from_str(input: &str) -> Result<Self> {
    if let Ok(addr) = IpAddr::from_str(input) {
      return Ok(IpMatchCondition::Addr(addr));
    }
    if let Ok(cidr) = IpNetwork::from_str(input) {
      return Ok(IpMatchCondition::Cidr(cidr));
    }
    strip_index_variant!(input, "geoip:", IpMatchCondition::GeoIp);
    Err(anyhow!("Invalid IP condition: {}", input))
  }
}

pub fn serialize_regex_set<'a, S: Serializer>(
  this: &'a RegexSet,
  serializer: S,
) -> Result<S::Ok, S::Error> {
  this.patterns().serialize(serializer)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainMatcher {
  /// `include`
  included: Vec<SmolStr>,
  /// `regex:`
  #[serde(
    deserialize_with = "serde_regex::deserialize",
    serialize_with = "serialize_regex_set"
  )]
  regex: RegexSet,
  /// `full:`
  full: Vec<SmolStr>,
  /// Rules without prefix
  contains: Vec<SmolStr>,
  /// `domain:`
  domain: Vec<SmolStr>,
}

impl DomainMatcher {
  pub fn new<I>(rules: I) -> Result<Self>
  where
    I: IntoIterator<Item = SmolStr>,
  {
    let mut included = vec![];
    let mut regex: Vec<String> = vec![];
    let mut full = vec![];
    let mut contains = vec![];
    let mut domain = vec![];

    for rule in rules {
      if let Some(res) = rule.strip_prefix("include:") {
        included.push(res.into());
        continue;
      }
      if let Some(res) = rule.strip_prefix("regex:") {
        regex.push(res.into());
        continue;
      }
      if let Some(res) = rule.strip_prefix("full:") {
        full.push(res.into());
        continue;
      }
      if let Some(res) = rule.strip_prefix("domain:") {
        domain.push(format!(".{}", res).into());
        continue;
      }
      contains.push(rule.into());
    }
    Ok(DomainMatcher {
      included,
      regex: RegexSet::new(regex)?,
      full,
      contains,
      domain,
    })
  }

  pub fn is_match(&self, domain: &str) -> bool {
    for pat in &self.contains {
      let pat: &str = pat.borrow();
      if domain.contains(pat) {
        return true;
      }
    }
    for pat in &self.full {
      let pat: &str = pat.borrow();
      if domain == pat {
        return true;
      }
    }
    for pat in &self.domain {
      let pat: &str = pat.borrow();
      if domain.ends_with(pat) {
        return true;
      }
    }
    if self.regex.is_match(domain) {
      return true;
    }
    false
  }
}

fn deserialize_domain_matcher_text<'de, D>(d: D) -> Result<DomainMatcher, D::Error>
where
  D: Deserializer<'de>,
{
  let rules = <Vec<SmolStr>>::deserialize(d)?;

  match DomainMatcher::new(rules) {
    Ok(r) => Ok(r),
    Err(err) => Err(D::Error::custom(err)),
  }
}
