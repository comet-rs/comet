use crate::config::Config;
use crate::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64};

#[derive(Debug, Default)]
pub struct InboundMetrics {
  rx: AtomicU64,
  tx: AtomicU64,
}

#[derive(Debug, Default)]
pub struct OutboundMetrics {
  rx: AtomicU64,
  tx: AtomicU64,
  rtt: AtomicU32,
}

pub struct Metrics {
  inbounds: HashMap<SmolStr, InboundMetrics>,
  outbounds: HashMap<SmolStr, OutboundMetrics>,
}

impl Metrics {
  pub fn new(config: &Config) -> Self {
    let mut inbounds = HashMap::new();
    let mut outbounds = HashMap::new();

    for (tag, _inbound) in &config.inbounds {
      inbounds.insert(tag.clone(), InboundMetrics::default());
    }

    Self {
      inbounds,
      outbounds,
    }
  }
}
