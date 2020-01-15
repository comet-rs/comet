use anyhow::Result;
use async_trait::async_trait;
use common::connection::{AcceptedConnection, OutboundConnection};
use common::protocol::OutboundProtocol;

pub struct FreedomOutbound;

#[async_trait]
impl OutboundProtocol for FreedomOutbound {
    async fn connect<'a>(&self, conn: AcceptedConnection<'a>) -> Result<OutboundConnection<'a>> {
        // Ok(OutboundConnection::new(conn))
    }
}
