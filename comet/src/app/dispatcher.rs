use crate::prelude::*;
use anyhow::{bail, Context};
use std::time::Duration;
use tokio::stream::StreamExt;
use tokio::time::sleep;

pub async fn handle_tcp_conn(
  conn: &mut Connection,
  stream: ProxyStream,
  ctx: AppContextRef,
) -> Result<()> {
  let stream = if let Some(ref inbound_pipeline) = conn.inbound_pipeline {
    let inbound_pipeline = inbound_pipeline.clone();
    ctx
      .clone_plumber()
      .process(&inbound_pipeline, conn, stream, ctx.clone())
      .await
      .with_context(|| format!("When running inbound pipeline {}", inbound_pipeline))?
  } else {
    stream
  };

  info!("Accepted {}", conn);

  let outbound_tag = ctx.router.try_match(&conn, &ctx);

  let mut outbound = ctx
    .outbound_manager
    .connect(&outbound_tag, conn, &ctx)
    .await
    .with_context(|| format!("When connecting outbound {}", outbound_tag))?;

  if let Some(outbound_pipeline) = ctx.outbound_manager.get_pipeline(outbound_tag)? {
    outbound = ctx
      .clone_plumber()
      .process(&outbound_pipeline, conn, outbound, ctx.clone())
      .await
      .with_context(|| format!("When running outbound pipeline {}", outbound_pipeline))?;
  }

  match (stream, outbound) {
    (ProxyStream::Tcp(stream), ProxyStream::Tcp(outbound)) => {
      use futures::try_join;
      use tokio::io::{copy, split};

      let mut uplink = split(outbound);
      let mut downlink = split(stream);

      let c2s = copy(&mut downlink.0, &mut uplink.1);
      let s2c = copy(&mut uplink.0, &mut downlink.1);
      try_join!(c2s, s2c).context("When copying TCP stream")?;
    }
    (ProxyStream::Udp(mut stream), ProxyStream::Udp(mut outbound)) => loop {
      let mut sleep = sleep(Duration::from_secs(10));

      tokio::select! {
        Some(packet) = outbound.next() => {
          stream.send(packet).await?;
        },
        Some(packet) = stream.next() => {
          outbound.send(packet).await?;
        },
        _ = &mut sleep => break,
        else => break
      }
    },
    _ => bail!("Transport type mismatch"),
  }
  Ok(())
}
