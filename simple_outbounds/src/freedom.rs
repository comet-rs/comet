use anyhow::Result;
use async_trait::async_trait;
use common::connection::{AcceptedConnection, OutboundConnection};
use common::protocol::OutboundProtocol;
use common::RWPair;

pub struct FreedomOutbound;

#[async_trait]
impl OutboundProtocol for FreedomOutbound {
  async fn connect(
    &self,
    _conn: &mut AcceptedConnection,
    downlink: RWPair,
  ) -> Result<OutboundConnection> {
    Ok(OutboundConnection::new(RWPair::new(downlink)))
  }
}
