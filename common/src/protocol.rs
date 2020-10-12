use crate::connection::*;
use crate::RWPair;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait InboundProtocol: Sync {
    async fn accept<'a>(&self, conn: InboundConnection<'a>) -> Result<AcceptedConnection<'a>>;
}

#[async_trait]
pub trait OutboundProtocol {
    async fn connect<'a>(
        &self,
        conn: &mut AcceptedConnection<'_>,
        downlink: RWPair<'a>,
    ) -> Result<OutboundConnection<'a>>;
}
