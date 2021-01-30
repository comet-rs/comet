use std::path::PathBuf;

use anyhow::Context;

use crate::app::metrics::Metrics;
use crate::app::outbound_manager::OutboundManager;
use crate::app::plumber::Plumber;
use crate::config::Config;
use crate::dns::DnsService;
use crate::prelude::*;
use crate::router::Router;
use crate::{app::inbound_manager::InboundManager, rule_provider::RuleProviderManager};

#[cfg(target_os = "android")]
use crate::android::nat_manager::NatManager;

pub type AppContextRef = Arc<AppContext>;

pub struct AppContext {
    pub plumber: Arc<Plumber>,
    pub inbound_manager: Arc<InboundManager>,
    pub outbound_manager: OutboundManager,
    pub metrics: Metrics,
    pub router: Router,
    #[cfg(target_os = "android")]
    pub nat_manager: NatManager,
    pub dns: DnsService,
    pub rule_provider_manager: RuleProviderManager,
    pub data_dir: PathBuf,
}

impl AppContext {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(AppContext {
            plumber: Arc::new(Plumber::new(&config).with_context(|| "When creating plumber")?),
            inbound_manager: Arc::new(InboundManager::new(&config)),
            outbound_manager: OutboundManager::new(&config),
            metrics: Metrics::new(&config),
            router: Router::new(&config).with_context(|| "When creating router")?,
            #[cfg(target_os = "android")]
            nat_manager: NatManager::new(&config),
            dns: DnsService::new(&config).with_context(|| "When creating DNS server")?,
            rule_provider_manager: RuleProviderManager::new(&config)
                .with_context(|| "When creating rule provider")?,
            data_dir: config.data_dir.clone(),
        })
    }
}

macro_rules! ctx_impl_getter {
    ($fn:ident, $name:ident, $type:ident) => {
        pub fn $fn(&self) -> Arc<$type> {
            Arc::clone(&self.$name)
        }
    };
}

impl AppContext {
    ctx_impl_getter!(clone_plumber, plumber, Plumber);
    ctx_impl_getter!(clone_inbound_manager, inbound_manager, InboundManager);
}
