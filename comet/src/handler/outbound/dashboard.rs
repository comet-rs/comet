use super::{NewOutboundHandler, Outbound, OutboundAddr, OutboundHandler};
use crate::prelude::*;
use futures::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::io::DuplexStream;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio_compat_02::IoCompat;
use tokio_tungstenite::accept_async;
use tungstenite::Message;
use warp::{Filter, Server};

pub struct DashboardHandler {
  sender: Sender<Result<DuplexStream>>,
}

impl DashboardHandler {
  async fn handle_priv<S: AsyncRead + AsyncWrite + Unpin + 'static>(
    stream: S,
    ctx: AppContextRef,
  ) -> Result<()> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let mut interval = tokio::time::interval(Duration::from_millis(1000));
    loop {
      tokio::select! {
        Some(Ok(msg)) = ws_receiver.next() => {
          if msg.is_close() {
            break;
          }
        }
        _ = interval.next() => {
          let froze = ctx.metrics.freeze();
          let msg = Message::text(serde_json::to_string(&froze)?);
          ws_sender.send(msg).await?;
        }
        else => break
      };
    }
    Ok(())
  }
}

#[async_trait]
impl OutboundHandler for DashboardHandler {
  async fn handle(
    &self,
    _tag: &str,
    _conn: &mut Connection,
    ctx: &AppContextRef,
  ) -> Result<ProxyStream> {
    let (uplink, downlink) = tokio::io::duplex(1024);
    let ctx = ctx.clone();
    tokio::spawn(async move {
      let _ = Self::handle_priv(uplink, ctx).await;
      info!("Metrics connection closed");
    });
    // self.sender.send(Ok(uplink)).await?;

    Ok(RWPair::new(downlink).into())
  }

  fn port(&self) -> Option<u16> {
    None
  }
  fn addr(&self) -> Option<&OutboundAddr> {
    None
  }
}

impl NewOutboundHandler for DashboardHandler {
  fn new(_config: &Outbound) -> Self {
    let routes = warp::any().map(|| "Hello, World!");
    let server = warp::serve(routes);

    let (sender, receiver) = channel::<Result<DuplexStream>>(1);
    let incoming = receiver.map(|s| s.map(IoCompat::new));
    tokio::spawn(async move {
      server.run_incoming(incoming).await;
    });
    Self { sender }
  }
}
