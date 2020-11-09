use crate::config::Config;
use crate::prelude::*;
use serde::Serialize;
use serde::Serializer;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default, Serialize)]
pub struct MetricsValues {
  rx: AtomicUsize,
  tx: AtomicUsize,
  #[serde(serialize_with = "serialize_conn_count")]
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

fn serialize_conn_count<S: Serializer>(handle: &Arc<()>, serializer: S) -> Result<S::Ok, S::Error> {
  serializer.serialize_u64((Arc::strong_count(handle) - 1) as u64)
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
}
