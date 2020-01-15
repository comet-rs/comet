pub mod server;
use anyhow::Result;
use async_trait::async_trait;
use common::connection::{AcceptedConnection, InboundConnection};
use common::protocol::InboundProtocol;
use settings::inbound::InboundSocks5Settings;
use std::net::SocketAddr;

#[macro_use]
extern crate log;
use futures::try_join;

use async_std::net::ToSocketAddrs;
use common::Address;

use tokio::io::{copy, AsyncWriteExt};
use tokio::net::TcpStream;

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

pub async fn proxy(mut conn: AcceptedConnection<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let dest = match conn.dest_addr.addr {
        Address::Ip(ip) => SocketAddr::new(ip, conn.dest_addr.port),
        Address::Domain(domain) => {
            info!("Trying to resolve {}", domain.as_str());
            let dest_str = format!("{}:{}", domain.as_str(), conn.dest_addr.port);

            dest_str.to_socket_addrs().await?.next().unwrap()
        }
    };
    info!("Dest: {}", dest);
    let mut upstream = TcpStream::connect(dest).await?;

    if let Some(sniffer_data) = conn.sniffer_data.take() {
        upstream.write_all(&sniffer_data).await?;
    }

    let (mut outgoing_read, mut outgoing_write) = upstream.split();
    let c2s = copy(&mut conn.conn.read_half, &mut outgoing_write);
    let s2c = copy(&mut outgoing_read, &mut conn.conn.write_half);
    try_join!(c2s, s2c)?;
    info!("Done copying");
    Ok(())
}
