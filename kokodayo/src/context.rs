use crate::dns::DnsService;
use crate::android::nat_manager::NatManager;
use crate::app::inbound_manager::InboundManager;
use crate::app::metrics::Metrics;
use crate::app::outbound_manager::OutboundManager;
use crate::app::plumber::Plumber;
use crate::config::Config;
use crate::prelude::*;
use crate::router::Router;

pub type AppContextRef = Arc<AppContext>;

pub struct AppContext {
  pub plumber: Arc<Plumber>,
  pub inbound_manager: Arc<InboundManager>,
  pub outbound_manager: OutboundManager,
  pub metrics: Metrics,
  pub router: Router,
  pub nat_manager: NatManager,
  pub dns: DnsService,
}

impl AppContext {
  pub fn new(config: &Config) -> Result<Self> {
    Ok(AppContext {
      plumber: Arc::new(Plumber::new(config)?),
      inbound_manager: Arc::new(InboundManager::new(config)),
      outbound_manager: OutboundManager::new(config),
      metrics: Metrics::new(config),
      router: Router::new(config),
      nat_manager: NatManager::new(config),
      dns: DnsService::new(config)
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
