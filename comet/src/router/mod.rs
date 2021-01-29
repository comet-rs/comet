use crate::config::Config;
use crate::prelude::*;

pub mod matching;

#[derive(Debug, Deserialize, Clone)]
pub struct RouterConfig {
    #[serde(default)]
    pub rules: Vec<RouterRule>,
    pub defaults: RouterDefaults,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RouterDefaults {
    pub tcp: SmolStr,
    pub udp: Option<SmolStr>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all(deserialize = "snake_case"))]
pub struct RouterRule {
    pub to: SmolStr,
    pub rule: matching::MatchCondition,
}

pub struct Router {
    config: RouterConfig,
}

impl Router {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Router {
            config: config.router.clone(),
        })
    }

    pub fn try_match(&self, conn: &Connection, _ctx: &AppContextRef) -> &str {
        for rule in &self.config.rules {
            if rule.rule.is_match(conn) {
                return &rule.to;
            }
        }
        match conn.typ {
            TransportType::Tcp => &self.config.defaults.tcp,
            TransportType::Udp => self.config.defaults.udp.as_ref().unwrap(),
        }
    }
}
