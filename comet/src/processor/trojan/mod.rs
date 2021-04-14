use crate::prelude::*;
use anyhow::bail;
use url::{Host, Url};

mod client;
pub fn register(plumber: &mut Plumber) {
    client::register(plumber);
}

#[derive(Debug, Clone, Deserialize)]
pub struct BundleConfigInner {
    sni: Option<SmolStr>,
    password: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum BundleConfig {
    Config {
        server: Option<DestAddr>,
        #[serde(flatten)]
        inner: BundleConfigInner,
    },
    Url {
        url: String,
    },
}

pub fn parse_url(url: &str) -> Result<BundleConfig> {
    let mut server = DestAddr::default();
    let parsed = Url::parse(url)?;

    if parsed.scheme() != "trojan" {
        bail!("Not trojan URL");
    }

    let mut inner = BundleConfigInner {
        sni: None,
        password: parsed.username().into(),
    };

    match parsed.host() {
        Some(Host::Domain(s)) => {
            server.set_domain(s);
        }
        Some(Host::Ipv4(s)) => {
            server.set_ip(s);
        }
        Some(Host::Ipv6(s)) => {
            server.set_ip(s);
        }
        None => {
            bail!("No host in URL");
        }
    }
    server.set_port(parsed.port().unwrap_or(443));

    todo!();
}
