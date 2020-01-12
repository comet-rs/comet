use crate::connection::*;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait InboundProtocol {
    async fn accept<'a>(&self, conn: InboundConnection<'a>) -> Result<AcceptedConnection<'a>>;
}

#[async_trait]
pub trait OutboundProtocol {
    async fn connect<'a>(&self, conn: AcceptedConnection<'a>) -> Result<OutboundConnection<'a>>;
}
