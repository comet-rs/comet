use crate::config::Config;
use crate::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct MetricsValues {
  rx: AtomicU64,
  tx: AtomicU64,
}

impl MetricsValues {
  pub fn add_rx(&self, value: usize) {
    self.rx.fetch_add(value as u64, Ordering::SeqCst);
  }

  pub fn add_tx(&self, value: usize) {
    self.tx.fetch_add(value as u64, Ordering::SeqCst);
  }
}

#[derive(Debug)]
pub struct Metrics {
  inbounds: HashMap<SmolStr, Arc<MetricsValues>>,
  outbounds: HashMap<SmolStr, Arc<MetricsValues>>,
}

impl Metrics {
  pub fn new(config: &Config) -> Self {
    let mut inbounds = HashMap::new();
    let mut outbounds = HashMap::new();

    for (tag, _inbound) in &config.inbounds {
      inbounds.insert(tag.clone(), Arc::new(MetricsValues::default()));
    }

    for (tag, _outbound) in &config.outbounds {
      outbounds.insert(tag.clone(), Arc::new(MetricsValues::default()));
    }

    Self {
      inbounds,
      outbounds,
    }
  }

  pub fn get_inbound(&self, tag: &str) -> Option<Arc<MetricsValues>> {
    self.inbounds.get(tag).map(|v| v.clone())
  }

  pub fn get_outbound(&self, tag: &str) -> Option<Arc<MetricsValues>> {
    self.outbounds.get(tag).map(|v| v.clone())
  }
}
