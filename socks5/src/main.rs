use common::connection::AcceptedConnection;
use env_logger;
use socks5::InboundSocks5Protocol;
use std::net::SocketAddr;

#[macro_use]
extern crate log;
use futures::try_join;

use async_std::net::ToSocketAddrs;
use common::Address;

use tokio::io::{copy, AsyncWriteExt};
use tokio::net::TcpStream;

use transport::inbound::tcp::InboundTcpTransport;

async fn proxy(mut conn: AcceptedConnection<'_>) -> Result<(), Box<dyn std::error::Error>> {
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let mut transport = InboundTcpTransport::listen(
        SocketAddr::new([127, 0, 0, 1].into(), 8080),
        InboundSocks5Protocol {},
    )
    .await?;
    info!("Listening at 127.0.0.1:8080");

    loop {
        let conn = transport.accept().await?;

        tokio::spawn(async move {
            if let Err(e) = proxy(conn).await {
                info!("Error: {}", e)
            }
        });
    }
}
