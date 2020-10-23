pub mod app;
pub mod config;
pub mod context;
pub mod net_wrapper;
pub mod processor;
pub mod transport;

use crate::context::AppContext;
use crate::prelude::*;
use anyhow::{Context, Result};
use log::{error, info};
use std::sync::Arc;
use tokio::net::TcpStream;
use futures::try_join;

async fn handle_tcp_conn(
  conn: Connection,
  stream: RWPair,
  ctx: AppContextRef,
) -> Result<Connection> {
  let (conn, mut stream) = ctx
    .clone_plumber()
    .process_stream(&conn.inbound_tag.clone(), conn, stream, ctx)
    .await?;
  info!("Accepted {:?}", conn);
  let dest_addr = conn.dest_addr.as_ref().unwrap();
  let mut outbound =
    RWPair::new(TcpStream::connect((dest_addr.addr.to_string(), dest_addr.port)).await?);
  stream.bidi_copy(&mut outbound).await?;

  Ok(conn)
}

pub async fn run() -> Result<()> {
  let config = config::load_file("./config.yml")
    .await
    .with_context(|| "Failed to read config file")?;

  let ctx = Arc::new(AppContext::new(&config)?);
  let (mut tcp_conns, _udp_conns) = ctx.clone_inbound_manager().start().await?;

  let tcp_handle = tokio::spawn(async move {
    while let Some((conn, stream)) = tcp_conns.recv().await {
      let ctx = Arc::clone(&ctx);
      tokio::spawn(async move {
        match handle_tcp_conn(conn, stream, ctx).await {
          Ok(r) => {
            info!("Done handling {:?}", r);
          }
          Err(err) => {
            error!("Failed to handle accepted connection: {}", err);
          }
        }
      });
    }
  });

  try_join!(tcp_handle);
  Ok(())
}

pub mod prelude {
  pub use crate::app::plumber::Processor;
  pub use crate::context::AppContextRef;
  pub use anyhow::Result;
  pub use async_trait::async_trait;
  pub use common::*;
  pub use log::*;
  pub use serde::Deserialize;
  pub use smol_str::SmolStr;
  pub use std::sync::Arc;
  pub use tokio::prelude::*;
}
