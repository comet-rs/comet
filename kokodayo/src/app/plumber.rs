//! Pipeline related types and operations.

use crate::config::Config;
use crate::config::ProcessorConfig;
use crate::prelude::*;
use crate::processor;
use crate::AppContextRef;
use anyhow::Context;
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
    let pipeline = self.pipelines.get(tag).unwrap();
    Ok(pipeline.process(stream, conn, ctx).await?)
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
}

macro_rules! processor_item {
  ($($variant:ident => $processor:ty), *) => {
    pub enum ProcessorItem {
      $($variant(Arc<$processor>)),*
    }

    impl ProcessorItem {
      pub fn new(config: &ProcessorConfig) -> Result<Self> {
        Ok(match config {
          $(ProcessorConfig::$variant(c) => ProcessorItem::$variant(Arc::new(<$processor>::new(c)?))),*,
          _ => unimplemented!()
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
            ProcessorItem::$variant(ref s) =>
                  Arc::clone(s).process(stream, conn, ctx).await
          ),*
        }
      }
    }

    impl std::fmt::Debug for ProcessorItem {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
          $(ProcessorItem::$variant(_) => stringify!($variant)),*
        };
        write!(f, "ProcessorItem::{}", s)
      }
    }
  };
}

processor_item!(
  Sniffer => processor::sniffer::SnifferProcessor,
  Socks5ProxyServer => processor::socks5_proxy::Socks5ProxyServerProcessor,
  HttpProxyClient => processor::http_proxy::HttpProxyClientProcessor
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
