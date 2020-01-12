use ipnetwork::IpNetwork;
use regex::Regex;
use serde::Deserialize;
use serde::Deserializer;
use std::net::IpAddr;
use std::ops::Range;
use std::str::FromStr;

#[derive(Deserialize, Debug)]
pub enum DomainStrategy {
    AsIs,
    IPIfNonMatch,
    IPOnDemand,
}

impl Default for DomainStrategy {
    fn default() -> Self {
        Self::AsIs
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "lowercase"))]
pub enum RoutingProtocol {
    Http,
    Tls,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "lowercase"))]
pub enum RoutingNetwork {
    Tcp,
    Udp,
    #[serde(rename(deserialize = "tcp,udp"))]
    TcpUdp,
}

impl Default for RoutingNetwork {
    fn default() -> RoutingNetwork {
        Self::TcpUdp
    }
}

#[derive(Deserialize, Debug)]
pub struct RoutingBalancer {
    tag: String,
    selector: Vec<String>,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct RoutingSettings {
    #[serde(default)]
    domain_strategy: DomainStrategy,
    #[serde(default)]
    rules: Vec<RoutingRule>,
    #[serde(default)]
    balancers: Vec<RoutingBalancer>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "lowercase"))]
pub enum RuleType {
    Field,
}
impl Default for RuleType {
    fn default() -> Self {
        Self::Field
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "camelCase"))]
pub enum RoutingDest {
    OutboundTag(String),
    BalancerTag(String),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct RoutingRule {
    #[serde(default)]
    r#type: RuleType,
    #[serde(default)]
    domain: Vec<DomainRule>,
    #[serde(default)]
    ip: Vec<IpRule>,
    #[serde(deserialize_with = "deserialize_ports", default)]
    port: Vec<PortRule>,
    #[serde(default)]
    network: RoutingNetwork,
    #[serde(default)]
    source: Vec<IpRule>,
    #[serde(default)]
    user: Vec<String>,
    #[serde(default)]
    inbound_tag: Vec<String>,
    #[serde(default)]
    protocol: Vec<RoutingProtocol>,
    #[serde(flatten)]
    dest: RoutingDest,
}

#[derive(Debug)]
pub enum DomainRule {
    FindAny(String),
    FullDomain(String),
    SubDomain(String),
    GeoSite(String),
    RegExp(Regex),
    ExtDat { filename: String, tag: String },
}

impl<'de> Deserialize<'de> for DomainRule {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.len() == 0 {
            return Err(serde::de::Error::custom("Empty rule"));
        }
        let v: Vec<&str> = s.splitn(2, ":").collect();
        if v.len() == 1 {
            Ok(Self::FindAny(s))
        } else {
            match &v[0].to_lowercase()[..] {
                "full" => Ok(Self::FullDomain(v[1].trim().to_owned())),
                "domain" => Ok(Self::SubDomain(v[1].trim().to_owned())),
                "geosite" => Ok(Self::GeoSite(v[1].trim().to_owned())),
                "regexp" => match Regex::new(v[1].trim()) {
                    Ok(r) => Ok(Self::RegExp(r)),
                    Err(e) => Err(serde::de::Error::custom(format!("Invalid RegExp: {:?}", e))),
                },
                "ext" => match parse_ext(v[1]) {
                    Ok((filename, tag)) => Ok(Self::ExtDat {
                        filename: filename,
                        tag: tag,
                    }),
                    Err(reason) => Err(serde::de::Error::custom(reason)),
                },
                _ => Err(serde::de::Error::custom(format!("Invalid rule: {}", s))),
            }
        }
    }
}

#[derive(Debug)]
pub enum IpRule {
    Plain(IpAddr),
    Cidr(IpNetwork),
    GeoIP(String),
    ExtDat { filename: String, tag: String },
}

impl<'de> Deserialize<'de> for IpRule {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.len() == 0 {
            return Err(serde::de::Error::custom("Empty rule"));
        }
        let v: Vec<&str> = s.splitn(2, ":").collect();
        if v.len() == 1 {
            if let Ok(ip) = IpAddr::from_str(&s) {
                Ok(Self::Plain(ip))
            } else if let Ok(cidr) = IpNetwork::from_str(&s) {
                Ok(Self::Cidr(cidr))
            } else {
                return Err(serde::de::Error::custom(format!("Invalid rule: {}", s)));
            }
        } else {
            match &v[0].to_lowercase()[..] {
                "geoip" => Ok(Self::GeoIP(v[1].trim().to_owned())),
                "ext" => match parse_ext(v[1]) {
                    Ok((filename, tag)) => Ok(Self::ExtDat {
                        filename: filename,
                        tag: tag,
                    }),
                    Err(reason) => Err(serde::de::Error::custom(reason)),
                },
                _ => Err(serde::de::Error::custom(format!("Invalid rule: {}", s))),
            }
        }
    }
}

fn parse_ext(v: &str) -> Result<(String, String), String> {
    let p: Vec<&str> = v.splitn(2, ":").collect();
    if p.len() < 2 {
        Err(format!("Invalid ext rule: {}", v))
    } else {
        Ok((p[0].trim().to_owned(), p[1].trim().to_owned()))
    }
}

#[derive(Debug)]
pub enum PortRule {
    Port(u16),
    Range(Range<u16>),
}

fn deserialize_ports<'de, D>(deserializer: D) -> Result<Vec<PortRule>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let mut rules = Vec::new();

    for raw_rule in s.split(",") {
        let trimmed = raw_rule.trim();
        if let Ok(port) = u16::from_str(trimmed) {
            rules.push(PortRule::Port(port));
            continue;
        }
        let parts: Vec<&str> = raw_rule.split("-").collect();
        if parts.len() == 2 {
            match (
                u16::from_str(parts[0].trim()),
                u16::from_str(parts[1].trim()),
            ) {
                (Ok(min), Ok(max)) => {
                    rules.push(PortRule::Range(Range {
                        start: min,
                        end: max,
                    }));
                    continue;
                }
                _ => (),
            }
        }
        return Err(serde::de::Error::custom(format!(
            "Invalid port range rule: {}",
            raw_rule
        )));
    }

    Ok(rules)
}
