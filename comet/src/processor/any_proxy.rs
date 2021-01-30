use tokio_prepend_io::PrependReader;

use super::http::server::ServerProcessor as HttpProcessor;
use super::socks5_proxy::Socks5ProxyServerProcessor as Socks5Processor;
use crate::prelude::*;

pub fn register(plumber: &mut Plumber) {
    plumber.register("any_server", |_, _| {
        Ok(Box::new(AnyProxyServerProcessor {}))
    });
}
pub struct AnyProxyServerProcessor {}

#[async_trait]
impl Processor for AnyProxyServerProcessor {
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        conn: &mut Connection,
        ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        let mut stream = stream.into_tcp()?;
        let first_byte = stream.read_u8().await?;
        let prep = PrependReader::new(stream, &[first_byte][..]);
        let stream = RWPair::new(prep).into();

        match first_byte {
            4 | 5 => {
                Arc::new(Socks5Processor {})
                    .process(stream, conn, ctx)
                    .await
            }
            _ => Arc::new(HttpProcessor {}).process(stream, conn, ctx).await,
        }
    }
}
