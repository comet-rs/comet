use crate::config::Config;
use crate::prelude::*;
use anyhow::anyhow;

pub mod matching;

#[derive(Debug, Deserialize, Clone)]
pub struct RouterConfig {
    #[serde(default)]
    pub rules: Vec<RouterRule>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all(deserialize = "snake_case"))]
pub struct RouterRule {
    pub to: SmolStr,
    pub rule: Option<matching::MatchCondition>,
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

    pub fn try_match(&self, conn: &Connection, ctx: &AppContextRef) -> Result<&str> {
        for rule in &self.config.rules {
            let is_match = match &rule.rule {
                Some(rule) => rule.is_match(conn, ctx),
                None => true,
            };

            if is_match {
                info!("{} routed to {}", conn, &rule.to);
                return Ok(&rule.to);
            }
        }
        Err(anyhow!("No rules matched this connection"))
    }
}
