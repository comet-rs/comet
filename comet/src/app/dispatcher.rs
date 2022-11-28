use crate::prelude::*;
use anyhow::{bail, Context};
use std::time::Duration;
use tokio::time::sleep;
use tokio_stream::StreamExt;

static HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(4);

pub async fn handle_conn(
    conn: &mut Connection,
    stream: ProxyStream,
    ctx: AppContextRef,
) -> Result<()> {
    let ctx_clone = ctx.clone();
    let handshake_downlink_task = async move {
        // Inbound Pipeline
        let stream = if let Some(ref inbound_pipeline) = conn.inbound_pipeline {
            let inbound_pipeline = inbound_pipeline.clone();
            ctx_clone
                .clone_plumber()
                .process(&inbound_pipeline, conn, stream, ctx_clone.clone())
                .await
                .with_context(|| format!("running inbound pipeline {}", inbound_pipeline))?
        } else {
            stream
        };

        info!("Accepted {}", conn);

        // Routing
        let outbound_tag = ctx_clone
            .router
            .match_conn(conn, &ctx_clone)
            .await
            .to_owned();

        info!("Routed to {}", outbound_tag);

        Ok::<_, anyhow::Error>((stream, conn, outbound_tag))
    };

    let (stream, conn, outbound_tag) =
        tokio::time::timeout(HANDSHAKE_TIMEOUT, handshake_downlink_task)
            .await?
            .with_context(|| "downlink handshake failed")?;

    // Prepare, save original dest
    let dest_ori = conn.dest_addr.clone();
    if let Some(outbound_pipeline) = ctx.outbound_manager.get_pipeline(&outbound_tag)? {
        ctx.clone_plumber()
            .prepare(outbound_pipeline, conn, ctx.clone())
            .await
            .with_context(|| format!("preparing outbound pipeline {}", outbound_pipeline))?;
    }

    // Connect
    let mut outbound = ctx
        .outbound_manager
        .connect(&outbound_tag, conn, &ctx)
        .await
        .with_context(|| format!("connecting outbound {}", outbound_tag))?;

    // Restore dest
    conn.dest_addr = dest_ori;

    // Outbound Pipeline
    if let Some(outbound_pipeline) = ctx.outbound_manager.get_pipeline(&outbound_tag)? {
        outbound = ctx
            .clone_plumber()
            .process(outbound_pipeline, conn, outbound, ctx.clone())
            .await
            .with_context(|| format!("running outbound pipeline {}", outbound_pipeline))?;
    }

    // Bi-directional Copy
    match (stream, outbound) {
        (ProxyStream::Tcp(mut stream), ProxyStream::Tcp(mut outbound)) => {
            debug!("Downlink: {:?} Uplink: {:?}", stream, outbound);

            let (uplink_sent, downlink_sent) =
                tokio::io::copy_bidirectional(&mut stream, &mut outbound).await?;

            info!(
                "Connection ends, uplink sent {}, downlink sent {}",
                uplink_sent, downlink_sent
            );
        }
        (ProxyStream::Udp(mut stream), ProxyStream::Udp(mut outbound)) => loop {
            let mut sleep = Box::pin(sleep(Duration::from_secs(10)));

            tokio::select! {
              Some(packet) = outbound.next() => {
                if stream.send(packet).await.is_err() {
                  break;
                }
              },
              Some(packet) = stream.next() => {
                if outbound.send(packet).await.is_err() {
                  break;
                }
              },
              _ = &mut sleep => break,
              else => break
            }
        },
        _ => bail!("Transport type mismatch"),
    }
    Ok(())
}
