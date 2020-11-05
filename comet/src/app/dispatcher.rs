use crate::prelude::*;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::timeout;

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
  req: UdpRequest,
  ctx: AppContextRef,
) -> Result<Connection> {
  let (mut conn, mut req) = ctx
    .clone_plumber()
    .process_packet(&conn.inbound_pipeline.clone(), conn, req, ctx.clone())
    .await?;
  let dest_addr_ips = ctx.dns.resolve_addr(&conn.dest_addr).await?;

  let outbound_tag = ctx.router.try_match(&conn, &ctx);

  let outbound = ctx
    .outbound_manager
    .connect_udp(
      outbound_tag,
      &conn,
      SocketAddr::new(dest_addr_ips[0], conn.dest_addr.port_or_error()?),
      &ctx,
    )
    .await?;

  if let Some(outbound_pipeline) = ctx
    .outbound_manager
    .get_pipeline(outbound_tag, TransportType::Udp)?
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
