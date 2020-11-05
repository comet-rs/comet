//! Pipeline related types and operations.
use crate::config::Config;
use crate::prelude::*;
use crate::processor;
use crate::AppContextRef;
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

type NewProcessorFn = Box<dyn Fn(YamlValue) -> Result<Box<dyn Processor>> + Send + Sync>;

pub struct Plumber {
  pipelines: HashMap<SmolStr, Pipeline>,
  processors: HashMap<&'static str, NewProcessorFn>,
}

impl Plumber {
  pub fn new(config: &Config) -> Result<Self> {
    let mut this = Plumber {
      pipelines: HashMap::with_capacity(config.pipelines.len()),
      processors: HashMap::new(),
    };

    processor::do_register(&mut this);

    for (tag, pipeline) in &config.pipelines {
      this
        .pipelines
        .insert(tag.clone(), Pipeline::new(&this, pipeline)?);
    }

    Ok(this)
  }

  pub fn register<F>(&mut self, name: &'static str, new_fn: F)
  where
    F: Fn(YamlValue) -> Result<Box<dyn Processor>> + Send + Sync + 'static,
  {
    info!("Registering {}", name);
    self.processors.insert(name, Box::new(new_fn));
  }

  pub fn new_processor(
    &self,
    name: &str,
    config: YamlValue,
  ) -> Result<(&'static str, Box<dyn Processor>)> {
    let create_fn = self
      .processors
      .get_key_value(name)
      .ok_or_else(|| anyhow!("Processor {} not found", name))?;

    Ok((create_fn.0, create_fn.1(config)?))
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
  items: Vec<(&'static str, Arc<dyn Processor>)>,
}

impl Pipeline {
  pub fn new(plumber: &Plumber, processors: &[YamlValue]) -> Result<Self> {
    let items: Result<Vec<_>> = processors
      .iter()
      .map(|conf| {
        let mapping = conf.as_mapping().unwrap();
        let name = mapping
          .get(&YamlValue::String("type".to_string()))
          .unwrap()
          .as_str()
          .unwrap();
        plumber
          .new_processor(name, conf.clone())
          .map(|(n, p)| (n, Arc::from(p)))
      })
      .collect();

    Ok(Pipeline { items: items? })
  }

  pub async fn process(
    &self,
    mut stream: RWPair,
    mut conn: Connection,
    ctx: AppContextRef,
  ) -> Result<(Connection, RWPair)> {
    for item in &self.items {
      let result = item.1.clone().process(stream, &mut conn, ctx.clone()).await;
      stream = result.with_context(|| format!("Error running processor {}", item.0))?;
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
      let result = item
        .1
        .clone()
        .process_udp(req, &mut conn, ctx.clone())
        .await;
      req = result.with_context(|| format!("Error running processor {}", item.0))?;
    }
    Ok((conn, req))
  }
}

#[async_trait]
pub trait Processor: Send + Sync {
  async fn process(
    self: Arc<Self>,
    _stream: RWPair,
    _conn: &mut Connection,
    _ctx: AppContextRef,
  ) -> Result<RWPair> {
    Err(anyhow!("This processor doesn't support TCP"))
  }

  async fn process_udp(
    self: Arc<Self>,
    _req: UdpRequest,
    _conn: &mut Connection,
    _ctx: AppContextRef,
  ) -> Result<UdpRequest> {
    Err(anyhow!("This processor doesn't support UDP"))
  }
}