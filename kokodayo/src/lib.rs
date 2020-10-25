pub mod android;
pub mod app;
pub mod common;
pub mod config;
pub mod context;
pub mod dns;
pub mod net_wrapper;
pub mod processor;
pub mod router;
pub mod transport;
pub mod utils;

use crate::context::AppContext;
use crate::prelude::*;
use anyhow::Context;
use log::{error, info};
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

async fn handle_tcp_conn(
  conn: Connection,
  stream: RWPair,
  ctx: AppContextRef,
) -> Result<Connection> {
  let (mut conn, mut stream) = ctx
    .clone_plumber()
    .process_stream(&conn.inbound_pipeline.clone(), conn, stream, ctx.clone())
    .await?;

  info!("Accepted {:?}", conn);

  let dest_addr = conn.dest_addr.clone().unwrap();
  let dest_addr_ips = ctx.dns.resolve_addr(&dest_addr.addr).await?;

  let outbound_tag = ctx.router.try_match(&conn, &ctx);

  let mut outbound = ctx
    .outbound_manager
    .connect_tcp_multi(outbound_tag, dest_addr_ips, dest_addr.port, &ctx)
    .await?;
  info!("Connected outbound: {:?}", outbound_tag);

  if let Some(outbound_pipeline) = ctx
    .outbound_manager
    .get_pipeline(outbound_tag, TransportType::Tcp)
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

async fn handle_udp_conn(
  conn: Connection,
  req: UdpRequest,
  ctx: AppContextRef,
) -> Result<Connection> {
  let (mut conn, mut req) = ctx
    .clone_plumber()
    .process_packet(&conn.inbound_pipeline.clone(), conn, req, ctx.clone())
    .await?;
  let dest_addr = conn.dest_addr.clone().unwrap();
  let dest_addr_ips = ctx.dns.resolve_addr(&dest_addr.addr).await?;

  let outbound_tag = ctx.router.try_match(&conn, &ctx);

  let outbound = ctx
    .outbound_manager
    .connect_udp(
      outbound_tag,
      &conn,
      SocketAddr::new(dest_addr_ips[0], dest_addr.port),
      &ctx,
    )
    .await?;

  if let Some(outbound_pipeline) = ctx
    .outbound_manager
    .get_pipeline(outbound_tag, TransportType::Udp)
  {
    let ret = ctx
      .clone_plumber()
      .process_packet(outbound_pipeline, conn, req, ctx.clone())
      .await?;
    conn = ret.0;
    req = ret.1;
  }
  outbound.send(&req.packet).await?;

  let mut buffer = [0u8; 4096];
  let n = timeout(Duration::from_secs(10), outbound.recv(&mut buffer)).await??;
  req.socket.send_to(&buffer[0..n], conn.src_addr).await?;
  Ok(conn)
}

pub async fn run(ctx: AppContextRef) -> Result<()> {
  let ctx1 = ctx.clone();
  let (mut tcp_conns, mut udp_conns) = ctx.clone_inbound_manager().start(ctx.clone()).await?;

  let ctx_tcp = ctx.clone();
  let _tcp_handle = tokio::spawn(async move {
    while let Some((conn, stream)) = tcp_conns.recv().await {
      let ctx = ctx_tcp.clone();
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

  let ctx_udp = ctx.clone();
  let _udp_handle = tokio::spawn(async move {
    while let Some((conn, req)) = udp_conns.recv().await {
      let ctx = ctx_udp.clone();
      tokio::spawn(async move {
        match handle_udp_conn(conn, req, ctx).await {
          Ok(_) => {}
          Err(err) => {
            error!("Failed to handle accepted connection: {:?}", err);
          }
        }
      });
    }
  });

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
  run(ctx).await?;
  Ok(())
}

pub async fn run_android(fd: u16, config_path: &str, running: Arc<AtomicBool>) -> Result<()> {
  let config = config::load_file(config_path)
    .await
    .with_context(|| "Failed to read config file")?;
  let ctx = Arc::new(AppContext::new(&config)?);

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
  pub use log::*;
  pub use serde::Deserialize;
  pub use smol_str::SmolStr;
  pub use std::sync::Arc;
  pub use tokio::prelude::*;
}
