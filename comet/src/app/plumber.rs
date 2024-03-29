//! Pipeline related types and operations.
use crate::config::Config;
use crate::prelude::*;
use crate::processor;
use crate::AppContextRef;
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

type NewProcessorFn = Box<dyn Fn(YamlValue, &str) -> Result<Box<dyn Processor>> + Send + Sync>;

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
            this.pipelines
                .insert(tag.clone(), Pipeline::new(&this, tag, pipeline)?);
        }

        Ok(this)
    }

    pub fn register<F>(&mut self, name: &'static str, new_fn: F)
    where
        F: Fn(YamlValue, &str) -> Result<Box<dyn Processor>> + Send + Sync + 'static,
    {
        if self.processors.contains_key(name) {
            panic!("Duplicate processor {}", name);
        }
        debug!("Registering {}", name);
        self.processors.insert(name, Box::new(new_fn));
    }

    pub fn new_processor(
        &self,
        name: &str,
        config: YamlValue,
        pipe_tag: &str,
    ) -> Result<(&'static str, Box<dyn Processor>)> {
        let create_fn = self
            .processors
            .get_key_value(name)
            .ok_or_else(|| anyhow!("Processor {} not found", name))?;

        Ok((
            create_fn.0,
            create_fn.1(config, pipe_tag).with_context(|| format!("creating {}", name))?,
        ))
    }

    pub async fn process(
        self: Arc<Self>,
        tag: &str,
        conn: &mut Connection,
        stream: ProxyStream,
        ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        self.get_pipeline(tag)?.process(stream, conn, ctx).await
    }

    pub async fn prepare(
        self: Arc<Self>,
        tag: &str,
        conn: &mut Connection,
        ctx: AppContextRef,
    ) -> Result<()> {
        self.get_pipeline(tag)?.prepare(conn, ctx).await
    }

    pub fn get_pipeline(&self, tag: &str) -> Result<&Pipeline> {
        self.pipelines
            .get(tag)
            .ok_or_else(|| anyhow!("Pipeline {} not found", tag))
    }
}

pub struct Pipeline {
    items: Vec<(&'static str, Arc<dyn Processor>)>,
}

impl Pipeline {
    pub fn new(plumber: &Plumber, tag: &str, processors: &[YamlValue]) -> Result<Self> {
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
                    .new_processor(name, conf.clone(), tag)
                    .map(|(n, p)| (n, Arc::from(p)))
            })
            .collect();

        Ok(Pipeline { items: items? })
    }

    pub async fn process(
        &self,
        mut stream: ProxyStream,
        conn: &mut Connection,
        ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        for item in &self.items {
            let result = item.1.clone().process(stream, conn, ctx.clone()).await;
            stream = result.with_context(|| format!("running processor {}", item.0))?;
        }
        Ok(stream)
    }

    pub async fn prepare(&self, conn: &mut Connection, ctx: AppContextRef) -> Result<()> {
        for item in &self.items {
            item.1
                .clone()
                .prepare(conn, ctx.clone())
                .await
                .with_context(|| format!("preparing processor {}", item.0))?;
        }
        Ok(())
    }
}

#[async_trait]
pub trait Processor: Send + Sync {
    /// Prepares the context and connection before a outbound connection.
    /// DOES NOT RUN when used as inbound processor.
    ///
    /// Note: `dest_addr` set in this stage will be overwritten by the original value
    /// after outbound connection.
    ///
    /// Defaults to a no-op.
    async fn prepare(self: Arc<Self>, _conn: &mut Connection, _ctx: AppContextRef) -> Result<()> {
        Ok(())
    }

    /// Processing and wrapping of a proxy stream.
    ///
    /// Defaults to a no-op.
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        _conn: &mut Connection,
        _ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        Ok(stream)
    }
}
