pub mod server;
use anyhow::Result;
use async_trait::async_trait;
use common::connection::{AcceptedConnection, InboundConnection};
use common::protocol::InboundProtocol;
use settings::inbound::InboundSocks5Settings;

pub struct InboundSocks5Protocol;
impl InboundSocks5Protocol {
    pub fn new(_settings: &InboundSocks5Settings) -> Result<InboundSocks5Protocol> {
        Ok(InboundSocks5Protocol)
    }
}

#[async_trait]
impl InboundProtocol for InboundSocks5Protocol {
    async fn accept<'a>(&self, conn: InboundConnection<'a>) -> Result<AcceptedConnection<'a>> {
        let (conn, dest_addr) = server::serve(conn).await?;
        Ok(AcceptedConnection::new(conn.conn, conn.addr, dest_addr))
    }
}
