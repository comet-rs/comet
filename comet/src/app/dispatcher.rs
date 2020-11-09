use crate::prelude::*;
use anyhow::anyhow;
use std::time::Duration;
use tokio::stream::StreamExt;
use tokio::time::sleep;

pub async fn handle_tcp_conn(
  conn: Connection,
  stream: ProxyStream,
  ctx: AppContextRef,
) -> Result<Connection> {
  let (mut conn, stream) = ctx
    .clone_plumber()
    .process(&conn.inbound_pipeline.clone(), conn, stream, ctx.clone())
    .await?;

  info!("Accepted {}", conn);

  let outbound_tag = ctx.router.try_match(&conn, &ctx);

  let mut outbound = ctx
    .outbound_manager
    .connect(outbound_tag, &mut conn, &ctx)
    .await?;
  info!("Connected outbound: {}", outbound_tag);

  if let Some(outbound_pipeline) = ctx.outbound_manager.get_pipeline(outbound_tag)? {
    let ret = ctx
      .clone_plumber()
      .process(outbound_pipeline, conn, outbound.into(), ctx.clone())
      .await?;
    conn = ret.0;
    outbound = ret.1;
  }

  match (stream, outbound) {
    (ProxyStream::Tcp(stream), ProxyStream::Tcp(outbound)) => {
      use futures::try_join;
      use tokio::io::{copy, split};

      let mut uplink = split(outbound);
      let mut downlink = split(stream);

      let c2s = copy(&mut downlink.0, &mut uplink.1);
      let s2c = copy(&mut uplink.0, &mut downlink.1);
      try_join!(c2s, s2c)?;
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
    _ => return Err(anyhow!("Transport type mismatch")),
  }
  Ok(conn)
}
