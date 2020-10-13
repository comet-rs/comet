use crate::connection::*;
use crate::RWPair;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait InboundProtocol: Sync {
    async fn accept(&self, conn: InboundConnection) -> Result<AcceptedConnection>;
}

#[async_trait]
pub trait OutboundProtocol {
    async fn connect(
        &self,
        conn: &mut AcceptedConnection,
        downlink: RWPair,
    ) -> Result<OutboundConnection>;
}
