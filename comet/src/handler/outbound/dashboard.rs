use super::{NewOutboundHandler, Outbound, OutboundHandler};
use crate::prelude::*;
use futures::{SinkExt, StreamExt};
use once_cell::sync::OnceCell;
use std::time::Duration;
use tokio::io::DuplexStream;
use tokio::sync::mpsc::{channel, Sender};
use tokio_stream::wrappers::ReceiverStream;
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

pub struct DashboardHandler {
    sender: OnceCell<Sender<DuplexStream>>,
}

impl DashboardHandler {
    async fn handle_ws(ws: WebSocket, ctx: AppContextRef) -> Result<()> {
        let (mut ws_sender, mut ws_receiver) = ws.split();

        let mut interval = tokio::time::interval(Duration::from_millis(1000));
        loop {
            tokio::select! {
              Some(Ok(msg)) = ws_receiver.next() => {
                if msg.is_close() {
                  break;
                }
              }
              _ = interval.tick() => {
                let froze = ctx.metrics.freeze();
                let msg = Message::text(serde_json::to_string(&froze)?);
                ws_sender.send(msg).await?;
              }
              else => break
            };
        }

        Ok(())
    }

    async fn run_server<S: Stream<Item = DuplexStream> + Send>(incoming: S, ctx: AppContextRef) {
        let root =
            warp::path::end().map(|| warp::reply::html(include_str!("dashboard/index.html")));

        let ws = warp::path("ws")
            .and(warp::ws())
            .map(move |ws: warp::ws::Ws| {
                let ctx = ctx.clone();
                ws.on_upgrade(|websocket| async move {
                    let _ = Self::handle_ws(websocket, ctx).await;
                })
            });

        let routes = root.or(ws);
        let server = warp::serve(routes);

        server
            .run_incoming(incoming.map(|s| -> Result<_, std::convert::Infallible> { Ok(s) }))
            .await;
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
        let sender = self.sender.get_or_init(|| {
            let ctx = ctx.clone();
            let (sender, receiver) = channel(1);
            let incoming = ReceiverStream::new(receiver);
            tokio::spawn(async move {
                Self::run_server(incoming, ctx).await;
            });
            sender
        });
        sender.send(uplink).await?;

        Ok(RWPair::new(downlink).into())
    }
}

impl NewOutboundHandler for DashboardHandler {
    fn new(_config: &Outbound) -> Self {
        Self {
            sender: OnceCell::new(),
        }
    }
}
