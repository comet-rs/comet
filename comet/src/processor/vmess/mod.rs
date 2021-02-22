mod alter_id;
mod session;

use crate::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecurityType {
    Aes128Gcm,
    Chacha20Poly1305,
    Auto,
}

impl Default for SecurityType {
    fn default() -> Self {
        if cfg!(any(target_arch = "x86_64", target_arch = "aarch64")) {
            Self::Aes128Gcm
        } else {
            Self::Chacha20Poly1305
        }
    }
}

struct ClientProcessor {}

#[async_trait]
impl Processor for ClientProcessor {
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        _conn: &mut Connection,
        _ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        Ok(stream)
    }
}
