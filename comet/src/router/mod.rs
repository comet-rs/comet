use std::{fs::File, path::PathBuf};

use crate::{config::Config, protos::v2ray::config::GeoSiteList};
use crate::prelude::*;
use protobuf::Message;
pub mod matching;

#[derive(Debug, Deserialize, Clone)]
pub struct RouterConfig {
    #[serde(default)]
    pub rules: Vec<RouterRule>,
    pub defaults: RouterDefaults,
    #[serde(default)]
    pub files: HashMap<SmolStr, RouterData>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouterDefaults {
    pub tcp: SmolStr,
    pub udp: Option<SmolStr>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouterRule {
    pub target: SmolStr,
    pub rule: matching::MatchCondition,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    #[serde(rename = "v2ray_geoip")]
    V2rayGeoIP,
    #[serde(rename = "v2ray_geosite")]
    V2rayGeoSite,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouterData {
    pub format: DataFormat,
    pub path: PathBuf,
}

pub struct Router {
    config: RouterConfig,
}

impl Router {
    pub fn new(config: &Config) -> Result<Self> {
        let router_config = config.router.clone();

        for (_, item) in &router_config.files {
            let path = if item.path.is_absolute() {
                item.path.clone()
            } else {
                let mut p = config.data_dir.clone();
                p.push(&item.path);
                p
            };

            let mut file = File::open(path)?;
            match item.format {
                DataFormat::V2rayGeoIP => {}
                DataFormat::V2rayGeoSite => {
                    let parsed = GeoSiteList::parse_from_reader(&mut file)?;
                    dbg!(&parsed.entry[0]);
                }
            }
        }

        Ok(Router {
            config: router_config,
        })
    }

    pub fn try_match(&self, conn: &Connection, _ctx: &AppContextRef) -> &str {
        for rule in &self.config.rules {
            if rule.rule.is_match(conn) {
                return &rule.target;
            }
        }
        match conn.typ {
            TransportType::Tcp => &self.config.defaults.tcp,
            TransportType::Udp => self.config.defaults.udp.as_ref().unwrap(),
        }
    }
}
