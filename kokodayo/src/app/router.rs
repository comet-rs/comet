use anyhow::{anyhow, Result};
use common::Connection;
use ipnetwork::IpNetwork;
use regex::Regex;
use std::net::IpAddr;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum MatchCondition {
  Any(Vec<MatchCondition>),
  All(Vec<MatchCondition>),
  DestAddr(IpMatchCondition),
  SrcAddr(IpMatchCondition),
  DestDomain(DomainMatchCondition),
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
      MatchCondition::DestAddr(cond) => {
        if let Some(addr) = &conn.dest_addr {
          if let common::Address::Ip(ip) = &addr.addr {
            return cond.is_match(ip);
          }
        }
        false
      }
      MatchCondition::SrcAddr(cond) => cond.is_match(&conn.src_addr.ip()),
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub enum DomainMatchCondition {
  Contains(String),
  Regex(Regex),
  Subdomain(String),
  Full(String),
  GeoSite(String),
}

impl DomainMatchCondition {
  pub fn is_match(&self, addr: &str) -> bool {
    match self {
      DomainMatchCondition::Contains(p) => addr.contains(p),
      DomainMatchCondition::Regex(r) => r.is_match(addr),
      DomainMatchCondition::Subdomain(p) => addr.ends_with(p),
      DomainMatchCondition::Full(p) => p == addr,
      DomainMatchCondition::GeoSite(_loc) => false,
    }
  }
}

impl FromStr for DomainMatchCondition {
  type Err = anyhow::Error;
  fn from_str(input: &str) -> Result<Self> {
    if let Some(res) = input.strip_prefix("domain:") {
      return Ok(DomainMatchCondition::Subdomain(format!(".{}", res)));
    }
    strip_index_variant!(input, "full:", DomainMatchCondition::Full);
    strip_index_variant!(input, "geosite:", DomainMatchCondition::GeoSite);
    if let Some(res) = input.strip_prefix("regexp:") {
      return Ok(DomainMatchCondition::Regex(Regex::new(res)?));
    }
    Ok(DomainMatchCondition::Contains(input.into()))
  }
}
