use crate::config::Config;
use crate::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
pub struct MetricsValues {
  rx: AtomicUsize,
  tx: AtomicUsize,
  conn_handle: Arc<()>,
}

impl MetricsValues {
  pub fn add_rx(&self, value: usize) {
    self.rx.fetch_add(value, Ordering::SeqCst);
  }

  pub fn add_tx(&self, value: usize) {
    self.tx.fetch_add(value, Ordering::SeqCst);
  }

  pub fn clone_conn_handle(&self) -> Arc<()> {
    self.conn_handle.clone()
  }
}

impl std::fmt::Debug for MetricsValues {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    write!(
      f,
      "Rx: {:?}, Tx: {:?}, Conn: {}",
      self.rx,
      self.tx,
      Arc::strong_count(&self.conn_handle) - 1
    )
  }
}

#[derive(Debug)]
pub struct Metrics {
  inbounds: HashMap<SmolStr, Arc<MetricsValues>>,
  outbounds: HashMap<SmolStr, Arc<MetricsValues>>,
}

#[derive(Default, Serialize)]
pub struct FrozeMetricsValues {
  rx: usize,
  tx: usize,
  conn_count: usize,
}

#[derive(Default, Serialize)]
pub struct FrozeMetrics<'k> {
  inbounds: HashMap<&'k str, FrozeMetricsValues>,
  outbounds: HashMap<&'k str, FrozeMetricsValues>,
}

impl Metrics {
  pub fn new(config: &Config) -> Self {
    let mut inbounds = HashMap::new();
    let mut outbounds = HashMap::new();

    for (tag, inbound) in &config.inbounds {
      if inbound.metering {
        inbounds.insert(tag.clone(), Arc::new(MetricsValues::default()));
      }
    }

    for (tag, outbound) in &config.outbounds {
      if outbound.metering {
        outbounds.insert(tag.clone(), Arc::new(MetricsValues::default()));
      }
    }

    Self {
      inbounds,
      outbounds,
    }
  }

  pub fn get_inbound(&self, tag: &str) -> Option<Arc<MetricsValues>> {
    self.inbounds.get(tag).cloned()
  }

  pub fn get_outbound(&self, tag: &str) -> Option<Arc<MetricsValues>> {
    self.outbounds.get(tag).cloned()
  }

  pub fn freeze(&self) -> FrozeMetrics {
    FrozeMetrics {
      inbounds: self
        .inbounds
        .iter()
        .map(|(name, value)| {
          (
            name.as_str(),
            FrozeMetricsValues {
              rx: value.rx.load(Ordering::Relaxed),
              tx: value.tx.load(Ordering::Relaxed),
              conn_count: Arc::strong_count(&value.conn_handle) - 1,
            },
          )
        })
        .collect(),
      outbounds: self
        .outbounds
        .iter()
        .map(|(name, value)| {
          (
            name.as_str(),
            FrozeMetricsValues {
              rx: value.rx.load(Ordering::Relaxed),
              tx: value.tx.load(Ordering::Relaxed),
              conn_count: Arc::strong_count(&value.conn_handle) - 1,
            },
          )
        })
        .collect(),
    }
  }
}
