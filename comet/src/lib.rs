pub mod app;
pub mod common;
pub mod config;
pub mod context;
pub mod crypto;
pub mod dns;
pub mod net_wrapper;
pub mod processor;
pub mod router;
pub mod transport;
pub mod utils;

#[cfg(target_os = "android")]
pub mod android;

use crate::app::dispatcher;
use crate::context::AppContext;
use crate::prelude::*;

use anyhow::Context;
use std::sync::Arc;
use std::time::Duration;

pub async fn run(ctx: AppContextRef) -> Result<()> {
  let (mut tcp_conns, mut udp_conns) = ctx.clone_inbound_manager().start(ctx.clone()).await?;

  let ctx_tcp = ctx.clone();
  let _tcp_handle = tokio::spawn(async move {
    while let Some((conn, stream)) = tcp_conns.recv().await {
      let ctx = ctx_tcp.clone();
      tokio::spawn(async move {
        match dispatcher::handle_tcp_conn(conn, stream, ctx).await {
          Ok(_) => {}
          Err(err) => {
            error!("Failed to handle accepted connection: {}", err);
          }
        }
      });
    }
  });

  let ctx_udp = ctx.clone();
  let _udp_handle = tokio::spawn(async move {
    while let Some((conn, req)) = udp_conns.recv().await {
      let ctx = ctx_udp.clone();
      tokio::spawn(async move {
        match dispatcher::handle_udp_conn(conn, req, ctx).await {
          Ok(_) => {}
          Err(err) => {
            error!("Failed to handle accepted connection: {}", err);
          }
        }
      });
    }
  });

  let ctx1 = ctx.clone();
  tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    loop {
      interval.tick().await;
      println!("{:?}", ctx1.metrics);
    }
  });
  Ok(())
}

pub async fn run_bin() -> Result<()> {
  let config = config::load_file("./config.yml")
    .await
    .with_context(|| "Failed to read config file")?;
  println!("{:#?}", config);
  let ctx = Arc::new(AppContext::new(&config)?);
  drop(config);
  
  run(ctx).await?;
  Ok(())
}

#[cfg(target_os = "android")]
pub async fn run_android(
  fd: u16,
  config_path: &str,
  uid_map: HashMap<u16, SmolStr>,
  running: Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
  info!("{:?}", uid_map);
  let config = config::load_file(config_path)
    .await
    .with_context(|| "Failed to read config file")?;
  let ctx = Arc::new(AppContext::new(&config)?);
  drop(config);

  let ctx1 = ctx.clone();
  std::thread::spawn(move || match android::nat::run_router(fd, ctx1, running) {
    Ok(_) => info!("Android router exited"),
    Err(err) => error!("Android router failed: {}", err),
  });

  run(ctx).await?;
  Ok(())
}

pub mod prelude {
  pub use crate::app::plumber::Processor;
  pub use crate::common::*;
  pub use crate::context::AppContextRef;
  pub use anyhow::Result;
  pub use async_trait::async_trait;
  pub use bytes::*;
  pub use log::*;
  pub use serde::Deserialize;
  pub use smol_str::SmolStr;
  pub use std::collections::HashMap;
  pub use std::pin::Pin;
  pub use std::sync::Arc;
  pub use std::task::Poll;
  pub use tokio::prelude::*;
  pub use serde_yaml::{from_value, Value as YamlValue, Mapping};
  pub use crate::app::plumber::Plumber;
}
