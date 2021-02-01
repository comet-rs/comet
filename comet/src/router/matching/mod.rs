use crate::prelude::*;
use futures::{Future, StreamExt};
use ipnetwork::IpNetwork;
use std::net::IpAddr;
use tokio_stream::StreamExt as TokioStreamExt;

mod domain;
use domain::DomainCondition;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all(deserialize = "snake_case"))]
pub enum MatchCondition {
    Any(Vec<MatchCondition>),
    All(Vec<MatchCondition>),

    DestIp(IpMatchCondition),
    SrcIp(IpMatchCondition),

    DestPort(PortCondition),

    Domain(DomainCondition),

    Transport(TransportType),

    InboundName(SmolStr),
    Provider(SmolStr),
}

impl MatchCondition {
    pub fn is_match<'a>(
        &'a self,
        conn: &'a Connection,
        ctx: &'a AppContextRef,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>> {
        let fut = async move {
            match self {
                MatchCondition::Any(conds) => {
                    tokio_stream::iter(conds.iter())
                        .then(|cond| cond.is_match(conn, ctx))
                        .any(|x| x)
                        .await
                }
                MatchCondition::All(conds) => {
                    tokio_stream::iter(conds.iter())
                        .then(|cond| cond.is_match(conn, ctx))
                        .all(|x| x)
                        .await
                }

                MatchCondition::DestIp(cond) => {
                    if let Some(ip) = &conn.dest_addr.ip {
                        return cond.is_match(ip);
                    }
                    false
                }
                MatchCondition::SrcIp(cond) => cond.is_match(&conn.src_addr.ip()),

                MatchCondition::Domain(cond) => {
                    if let Some(domain) = &conn.dest_addr.domain {
                        return cond.is_match(domain);
                    }
                    false
                }
                MatchCondition::Transport(t) => &conn.typ == t,
                MatchCondition::InboundName(name) => &conn.inbound_tag == name,
                MatchCondition::DestPort(cond) => {
                    if let Some(port) = &conn.dest_addr.port {
                        return cond.is_match(*port);
                    }
                    false
                }
                MatchCondition::Provider(s) => ctx.rule_provider.is_match("geosite", s, conn).await,
            }
        };
        Box::pin(fut)
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
