use crate::config::RouterRule;
use crate::config::Config;
use crate::prelude::*;
pub mod matching;

pub struct Router {
  rules: Vec<RouterRule>
}

impl Router {
  pub fn new(config: &Config) -> Self {
    Router {
      rules: config.router.rules.clone()
    }
  }

  pub fn try_match(&self, conn: &Connection, ctx: &AppContextRef) -> Option<&str> {
    for rule in &self.rules {
      if rule.rule.is_match(conn) {
        return Some(&rule.target)
      }
    }
    None
  }
}