use crate::sniffer::sniff;
use anyhow::Result;
use common::connection::{AcceptedConnection, InboundConnection};
use common::protocol::InboundProtocol;
use common::RWPair;
use log::info;
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub struct InboundTcpTransport {
    listener: TcpListener,
    protocol: Box<dyn InboundProtocol>,
}

impl InboundTcpTransport {
    pub async fn listen<P: InboundProtocol + 'static>(
        addr: SocketAddr,
        protocol: P,
    ) -> Result<Self> {
        Ok(InboundTcpTransport {
            listener: TcpListener::bind(addr).await?,
            protocol: Box::new(protocol),
        })
    }

    pub async fn accept(&mut self) -> Result<AcceptedConnection<'static>> {
        let (socket, addr) = self.listener.accept().await?;
        info!("Accepted {}", addr);
        let conn = InboundConnection {
            addr: addr,
            conn: RWPair::new(socket),
        };
        let mut handled = self.protocol.accept(conn).await?;
        let (cached_payload, sniff_result) = sniff(&mut handled.conn).await?;
        handled.sniffer_data = Some(cached_payload);
        handled.sniffed_dest = sniff_result;
        info!("Handled {:?}", handled);

        Ok(handled)
    }
}
