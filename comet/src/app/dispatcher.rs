use crate::prelude::*;
use std::time::Duration;
use tokio::stream::StreamExt;
use tokio::time::sleep;

pub async fn handle_tcp_conn(
  conn: Connection,
  stream: RWPair,
  ctx: AppContextRef,
) -> Result<Connection> {
  let (mut conn, stream) = ctx
    .clone_plumber()
    .process_stream(&conn.inbound_pipeline.clone(), conn, stream, ctx.clone())
    .await?;

  info!("Accepted {}", conn);

  let outbound_tag = ctx.router.try_match(&conn, &ctx);

  let mut outbound = ctx
    .outbound_manager
    .connect_tcp_multi(outbound_tag, &mut conn, &ctx)
    .await?;
  info!("Connected outbound: {}", outbound_tag);

  if let Some(outbound_pipeline) = ctx
    .outbound_manager
    .get_pipeline(outbound_tag, TransportType::Tcp)?
  {
    let ret = ctx
      .clone_plumber()
      .process_stream(outbound_pipeline, conn, outbound, ctx.clone())
      .await?;
    conn = ret.0;
    outbound = ret.1;
  }

  {
    use futures::try_join;
    use tokio::io::{copy, split};

    let mut uplink = split(outbound);
    let mut downlink = split(stream);

    let c2s = copy(&mut downlink.0, &mut uplink.1);
    let s2c = copy(&mut uplink.0, &mut downlink.1);
    try_join!(c2s, s2c)?;
  }

  Ok(conn)
}

pub async fn handle_udp_conn(
  conn: Connection,
  stream: UdpStream,
  ctx: AppContextRef,
) -> Result<Connection> {
  let (mut conn, mut stream) = ctx
    .clone_plumber()
    .process_udp(&conn.inbound_pipeline.clone(), conn, stream, ctx.clone())
    .await?;

  info!("Accepted {}", conn);

  let outbound_tag = ctx.router.try_match(&conn, &ctx);

  let outbound = ctx
    .outbound_manager
    .connect_udp(outbound_tag, &conn, &ctx)
    .await?;

  if let Some(outbound_pipeline) = ctx
    .outbound_manager
    .get_pipeline(outbound_tag, TransportType::Udp)?
  {
    let ret = ctx
      .clone_plumber()
      .process_udp(outbound_pipeline, conn, stream, ctx.clone())
      .await?;
    conn = ret.0;
    stream = ret.1;
  }

  loop {
    let mut buffer = [0u8; 4096];
    let mut sleep = sleep(Duration::from_secs(10));

    tokio::select! {
      Ok(n) = outbound.recv(&mut buffer) => {
        stream.send(BytesMut::from(&buffer[..n])).await?;
      },
      Some(packet) = stream.next() => {
        outbound.send(&packet).await?;
      },
      _ = &mut sleep => break,
      else => break
    }
  }

  Ok(conn)
}
