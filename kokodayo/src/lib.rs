pub mod app;
pub mod config;
pub mod net_wrapper;
pub mod processor;
pub mod transport;

use crate::app::plumber::Plumber;
use crate::prelude::*;

use crate::app::transport::start_inbounds;
use anyhow::{Context, Result};
use log::error;
use std::sync::Arc;

pub struct AppContext {}
pub type AppContextRef = Arc<AppContext>;

pub async fn run() -> Result<()> {
  let config = config::load_file("./config.yml")
    .await
    .with_context(|| "Failed to read config file")?;

  let plumber = Arc::new(Plumber::new(&config)?);
  let ctx = Arc::new(AppContext {});

  let (mut tcp_conns, _udp_conns) = start_inbounds(&config).await?;
  tokio::spawn(async move {
    while let Some((conn, stream)) = tcp_conns.recv().await {
      let plumber = Arc::clone(&plumber);
      let ctx = Arc::clone(&ctx);
      tokio::spawn(async move {
        let (stream, conn) = plumber
          .process_stream(&conn.inbound_tag.clone(), conn, RWPair::new(stream), ctx)
          .await
          .unwrap();
        println!("{:?}", conn);
      });
    }
    error!("All TCP inbounds has stopped");
  })
  .await?;

  Ok(())
}

pub mod prelude {
  pub use crate::app::plumber::Processor;
  pub use crate::AppContextRef;
  pub use anyhow::Result;
  pub use async_trait::async_trait;
  pub use common::*;
  pub use serde::Deserialize;
  pub use smol_str::SmolStr;
  pub use std::sync::Arc;
  pub use tokio::prelude::*;
}
