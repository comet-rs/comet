pub mod app;
pub mod config;
pub mod context;
pub mod net_wrapper;
pub mod processor;
pub mod router;
pub mod transport;
pub mod utils;
pub mod common;

use crate::context::AppContext;
use crate::prelude::*;
use anyhow::{anyhow, Context, Result};
use log::{error, info};
use std::borrow::Borrow;
use std::sync::Arc;
use std::time::Duration;

async fn handle_tcp_conn(
  conn: Connection,
  stream: RWPair,
  ctx: AppContextRef,
) -> Result<Connection> {
  let (mut conn, mut stream) = ctx
    .clone_plumber()
    .process_stream(&conn.inbound_tag.clone(), conn, stream, ctx.clone())
    .await?;

  info!("Accepted {:?}", conn);

  let dest_addr = conn.dest_addr.clone().unwrap();
  let dest_addr_ip = match &dest_addr.addr {
    Address::Ip(ip) => ip.clone(),
    Address::Domain(s) => {
      let s: &str = s.borrow();
      tokio::net::lookup_host((s, 443))
        .await?
        .nth(0)
        .ok_or_else(|| anyhow!("Unable to resolve"))?
        .ip()
    }
  };

  let routing_result = ctx.router.try_match(&conn, &ctx);

  let (outbound_tag, mut outbound) = ctx
    .outbound_manager
    .connect_tcp(routing_result, dest_addr_ip, dest_addr.port, &ctx)
    .await?;
  info!("Connected outbound: {:?}", outbound_tag);

  if let Some(outbound_pipeline) = ctx
    .outbound_manager
    .get_pipeline(outbound_tag, config::TransportType::Tcp)
  {
    let ret = ctx
      .clone_plumber()
      .process_stream(outbound_pipeline, conn, outbound, ctx.clone())
      .await?;
    conn = ret.0;
    outbound = ret.1;
  }

  stream.bidi_copy(&mut outbound).await?;
  Ok(conn)
}

pub async fn run() -> Result<()> {
  let config = config::load_file("./config.yml")
    .await
    .with_context(|| "Failed to read config file")?;
  println!("{:#?}", config);
  let ctx = Arc::new(AppContext::new(&config)?);
  let ctx1 = ctx.clone();
  let (mut tcp_conns, _udp_conns) = ctx.clone_inbound_manager().start(ctx.clone()).await?;

  let _tcp_handle = tokio::spawn(async move {
    while let Some((conn, stream)) = tcp_conns.recv().await {
      let ctx = Arc::clone(&ctx);
      tokio::spawn(async move {
        match handle_tcp_conn(conn, stream, ctx).await {
          Ok(r) => {
            info!("Done handling {:?}", r);
          }
          Err(err) => {
            error!("Failed to handle accepted connection: {:?}", err);
          }
        }
      });
    }
  });

  let mut interval = tokio::time::interval(Duration::from_secs(10));
  loop {
    interval.tick().await;
    println!("{:?}", ctx1.metrics);
  }
}

pub mod prelude {
  pub use crate::app::plumber::Processor;
  pub use crate::context::AppContextRef;
  pub use anyhow::Result;
  pub use async_trait::async_trait;
  pub use crate::common::*;
  pub use log::*;
  pub use serde::Deserialize;
  pub use smol_str::SmolStr;
  pub use std::sync::Arc;
  pub use tokio::prelude::*;
}
