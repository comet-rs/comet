use futures::{future, ready, Sink, StreamExt};
use tokio_tungstenite::client_async_with_config;
use tokio_util::io::StreamReader;
use tungstenite::{protocol::WebSocketConfig, Message};
use url::Url;

use crate::prelude::*;
use crate::utils::io::io_other_error;

pub fn register(plumber: &mut Plumber) {
    plumber.register("ws_client", |conf, _| {
        Ok(Box::new(ClientProcessor {
            config: from_value(conf)?,
        }))
    });
}

#[derive(Debug, Clone, Deserialize)]
struct ClientConfig {
    url: Url,
}

struct ClientProcessor {
    config: ClientConfig,
}

#[async_trait]
impl Processor for ClientProcessor {
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        _conn: &mut Connection,
        _ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        let stream = stream.into_tcp()?;

        let ws_config = WebSocketConfig {
            max_send_queue: Some(1),
            ..Default::default()
        };
        let (socket, _) =
            client_async_with_config(&self.config.url, stream, Some(ws_config)).await?;
        let (sink, stream) = socket.split();

        let stream = stream.filter_map(|msg| {
            let r = match msg {
                Ok(Message::Binary(data)) => Some(Ok(Bytes::from(data))),
                Ok(Message::Close(_)) => None,
                Ok(_) => Some(Err(io_other_error("unexpected message type"))),
                Err(e) => Some(Err(io_other_error(e))),
            };
            future::ready(r)
        });

        let reader = StreamReader::new(stream);
        let writer = WsWriter { inner: sink };

        Ok(RWPair::new_parts(reader, writer).into())
    }
}

struct WsWriter<W> {
    inner: W,
}

impl<W: Sink<Message> + Unpin> AsyncWrite for WsWriter<W>
where
    <W as Sink<Message>>::Error: Send + Sync + std::error::Error + 'static,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, futures_io::Error>> {
        ready!(Pin::new(&mut self.inner).poll_ready(cx)).map_err(io_other_error)?;

        let msg = Message::Binary(Vec::from(buf));
        match Pin::new(&mut self.inner).start_send(msg) {
            Err(e) => Poll::Ready(Err(io_other_error(e))),
            Ok(()) => {
                trace!("Written {:?}", buf);
                Poll::Ready(Ok(buf.len()))
            }
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), futures_io::Error>> {
        Poll::Ready(ready!(Pin::new(&mut self.inner).poll_flush(cx)).map_err(io_other_error))
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), futures_io::Error>> {
        Poll::Ready(ready!(Pin::new(&mut self.inner).poll_close(cx)).map_err(io_other_error))
    }
}
