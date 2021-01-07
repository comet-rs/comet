use crate::config::Config;
use crate::config::RouterConfig;
use crate::prelude::*;
pub mod matching;

pub struct Router {
    config: RouterConfig,
}

impl Router {
    pub fn new(config: &Config) -> Self {
        Router {
            config: config.router.clone(),
        }
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
