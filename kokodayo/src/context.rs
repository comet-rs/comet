use crate::prelude::*;
use crate::app::inbound_manager::InboundManager;
use crate::app::metrics::Metrics;
use crate::app::plumber::Plumber;
use crate::config::Config;

pub type AppContextRef = Arc<AppContext>;

pub struct AppContext {
  pub plumber: Arc<Plumber>,
  pub inbound_manager: Arc<InboundManager>,
  pub metrics: Metrics,
}

impl AppContext {
  pub fn new(config: &Config) -> Result<Self> {
    Ok(AppContext {
      plumber: Arc::new(Plumber::new(config)?),
      inbound_manager: Arc::new(InboundManager::new(config)),
      metrics: Metrics::new(config),
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

