//! Pipeline related types and operations.

use crate::config::Config;
use crate::config::ProcessorConfig;
use crate::prelude::*;
use crate::processor;
use crate::AppContextRef;
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

pub struct Plumber {
  pipelines: HashMap<SmolStr, Pipeline>,
}

impl Plumber {
  pub fn new(config: &Config) -> Result<Self> {
    let mut pipelines = HashMap::new();
    for (tag, pipeline) in &config.pipelines {
      pipelines.insert(tag.clone(), Pipeline::new(pipeline)?);
    }

    Ok(Plumber { pipelines })
  }

  pub async fn process_stream(
    self: Arc<Self>,
    tag: &str,
    conn: Connection,
    stream: RWPair,
    ctx: AppContextRef,
  ) -> Result<(Connection, RWPair)> {
    Ok(self.get_pipeline(tag)?.process(stream, conn, ctx).await?)
  }

  pub async fn process_packet(
    self: Arc<Self>,
    tag: &str,
    conn: Connection,
    req: UdpRequest,
    ctx: AppContextRef,
  ) -> Result<(Connection, UdpRequest)> {
    Ok(self.get_pipeline(tag)?.process_udp(req, conn, ctx).await?)
  }

  pub fn get_pipeline(&self, tag: &str) -> Result<&Pipeline> {
    self
      .pipelines
      .get(tag)
      .ok_or_else(|| anyhow!("Pipeline {} not found", tag))
  }
}

pub struct Pipeline {
  items: Vec<ProcessorItem>,
}

impl Pipeline {
  pub fn new(processors: &[ProcessorConfig]) -> Result<Self> {
    let items: Result<Vec<_>> = processors.iter().map(|x| ProcessorItem::new(x)).collect();
    Ok(Pipeline { items: items? })
  }
  pub async fn process(
    &self,
    mut stream: RWPair,
    mut conn: Connection,
    ctx: AppContextRef,
  ) -> Result<(Connection, RWPair)> {
    for item in &self.items {
      let result = item.process(stream, &mut conn, ctx.clone()).await;
      stream = result.with_context(|| format!("Error running processor {:?}", item))?;
    }
    Ok((conn, stream))
  }
  pub async fn process_udp(
    &self,
    mut req: UdpRequest,
    mut conn: Connection,
    ctx: AppContextRef,
  ) -> Result<(Connection, UdpRequest)> {
    for item in &self.items {
      let result = item.process_udp(req, &mut conn, ctx.clone()).await;
      req = result.with_context(|| format!("Error running processor {:?}", item))?;
    }
    Ok((conn, req))
  }
}

macro_rules! processor_item {
  ($($variant:ident => $processor:ty $([$($cond_key:ident $(= $cond_value:literal)?),*])?), *) => {
    pub enum ProcessorItem {
      $(
        $(#[cfg(any($($cond_key $(= $cond_value)?),*))])?
        $variant(Arc<$processor>)
      ),*
    }

    impl ProcessorItem {
      pub fn new(config: &ProcessorConfig) -> Result<Self> {
        Ok(match config {
          $(
            $(#[cfg(any($($cond_key $(= $cond_value)?),*))])?
            ProcessorConfig::$variant(c) => ProcessorItem::$variant(Arc::new(<$processor>::new(c)?))
          ),*,
        })
      }
      async fn process(
        &self,
        stream: RWPair,
        conn: &mut Connection,
        ctx: AppContextRef,
      ) -> Result<RWPair> {
        match *self {
          $(
            $(#[cfg(any($($cond_key $(= $cond_value)?),*))])?
            ProcessorItem::$variant(ref s) =>
                  Arc::clone(s).process(stream, conn, ctx).await
          ),*
        }
      }
    }

    impl std::fmt::Debug for ProcessorItem {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
          $(
            $(#[cfg(any($($cond_key $(= $cond_value)?),*))])?
            ProcessorItem::$variant(_) => stringify!($variant)
          ),*
        };
        write!(f, "ProcessorItem::{}", s)
      }
    }
  };
}

macro_rules! processor_item_udp {
  ($($variant:ident => $processor:ty $([$($cond_key:ident $(= $cond_value:literal)?),*])?), *) => {
    impl ProcessorItem {
      async fn process_udp(&self,
        req: UdpRequest,
        conn: &mut Connection,
        ctx: AppContextRef,) -> Result<UdpRequest> {
          match *self {
            $(
              $(#[cfg(any($($cond_key $(= $cond_value)?),*))])?
              ProcessorItem::$variant(ref s) =>
                    Arc::clone(s).process_udp(req, conn, ctx).await
            ),*,
            _ => unimplemented!()
          }
        }
    }
  };
}

processor_item!(
  Sniffer => processor::sniffer::SnifferProcessor,
  Socks5ProxyServer => processor::socks5_proxy::Socks5ProxyServerProcessor,
  HttpProxyClient => processor::http_proxy::HttpProxyClientProcessor,
  AndroidNat => processor::android::AndroidNatProcessor[target_os = "android"],
  AssociateUid => processor::unix::AssociateUidProcessor[target_os = "linux", target_os = "android"]
);

processor_item_udp!(
  AndroidNat => processor::android::AndroidNatProcessor[target_os = "android"],
  AssociateUid => processor::unix::AssociateUidProcessor[target_os = "linux", target_os = "android"]
);

#[async_trait]
pub trait Processor {
  async fn process(
    self: Arc<Self>,
    mut stream: RWPair,
    conn: &mut Connection,
    ctx: AppContextRef,
  ) -> Result<RWPair>;
}

#[async_trait]
pub trait UdpProcessor {
  async fn process_udp(
    self: Arc<Self>,
    mut req: UdpRequest,
    conn: &mut Connection,
    ctx: AppContextRef,
  ) -> Result<UdpRequest>;
}
