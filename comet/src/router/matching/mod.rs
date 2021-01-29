use crate::prelude::*;
use ipnetwork::IpNetwork;
use std::net::IpAddr;

mod domain;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all(deserialize = "snake_case"))]
pub enum MatchCondition {
    Any(Vec<MatchCondition>),
    All(Vec<MatchCondition>),

    DestIp(IpMatchCondition),
    SrcIp(IpMatchCondition),

    DestPort(PortCondition),

    #[serde(deserialize_with = "domain::deserialize_domain_matcher_text")]
    DestDomain(domain::DomainMatcher),

    InboundName(SmolStr),
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

            MatchCondition::DestIp(cond) => {
                if let Some(ip) = &conn.dest_addr.ip {
                    return cond.is_match(ip);
                }
                false
            }
            MatchCondition::SrcIp(cond) => {
                let ip = conn.src_addr.ip();
                cond.is_match(&ip)
            }

            MatchCondition::DestDomain(cond) => {
                if let Some(domain) = &conn.dest_addr.domain {
                    return cond.is_match(domain);
                }
                false
            }
            MatchCondition::InboundName(name) => &conn.inbound_tag == name,
            MatchCondition::Metadata => false,
            MatchCondition::DestPort(cond) => {
                if let Some(port) = &conn.dest_addr.port {
                    cond.is_match(*port)
                } else {
                    false
                }
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum IpMatchCondition {
    Addr(IpAddr),
    Cidr(IpNetwork),
}

impl IpMatchCondition {
    pub fn is_match(&self, addr: &IpAddr) -> bool {
        match self {
            IpMatchCondition::Addr(addr_) => addr_ == addr,
            IpMatchCondition::Cidr(net) => net.contains(*addr),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum PortCondition {
    Port(u16),
    Range(u16, u16),
}

impl PortCondition {
    pub fn is_match(&self, port: u16) -> bool {
        match self {
            Self::Port(expected) => *expected == port,
            Self::Range(l, r) => port >= *l && port <= *r,
        }
    }
}
