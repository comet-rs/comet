use matching::MatchMode;

use crate::config::Config;
use crate::prelude::*;

use self::matching::MatchCondition;

pub mod matching;

#[derive(Debug, Deserialize, Clone)]
pub struct RouterConfig {
    #[serde(default)]
    rules: Vec<RouterRule>,
    default: DefaultOut,
    #[serde(default)]
    resolve: Resolve,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum DefaultOut {
    Both(SmolStr),
    ByTransport { tcp: SmolStr, udp: SmolStr },
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Resolve {
    Never,
    IfNonMatch,
}

impl Default for Resolve {
    fn default() -> Self {
        Self::Never
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all(deserialize = "snake_case"))]
pub struct RouterRule {
    to: SmolStr,
    rule: matching::MatchCondition,
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

    pub async fn try_match_conn(
        &self,
        conn: &Connection,
        mode: MatchMode,
        ctx: &AppContextRef,
    ) -> Option<&str> {
        for rule in &self.config.rules {
            if rule.rule.is_match(conn, mode, ctx).await {
                return Some(&rule.to);
            }
        }
        None
    }

    pub async fn match_conn(&self, conn: &mut Connection, ctx: &AppContextRef) -> &str {
        // Match with domain or IP
        if let Some(res) = self.try_match_conn(conn, MatchMode::Any, ctx).await {
            return res;
        }
        if self.config.resolve == Resolve::IfNonMatch {
            debug!(
                "{} first match attempt unsuccessful, retrying with IP",
                conn
            );
            // If we have no IP...
            if conn.dest_addr.ip.is_none() {
                if let Ok(ips) = ctx.dns.resolve_addr(&conn.dest_addr, ctx).await {
                    for ip in &ips {
                        // Match again with IP in place
                        conn.dest_addr.ip = Some(*ip);
                        let res = self.try_match_conn(conn, MatchMode::IpOnly, ctx).await;
                        conn.dest_addr.ip = None; // Clear IP to not interfere with later operations

                        if let Some(res) = res {
                            return res;
                        }
                    }
                }
            }
        } else {
            debug!(
                "{} match attempt unsuccessful, falling back to default",
                conn
            );
        }

        match (&self.config.default, conn.typ) {
            (DefaultOut::Both(out), _) => out.as_str(),
            (DefaultOut::ByTransport { tcp, .. }, TransportType::Tcp) => tcp.as_str(),
            (DefaultOut::ByTransport { udp, .. }, TransportType::Udp) => udp.as_str(),
        }
    }
}
