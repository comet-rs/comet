#![allow(dead_code)]

use common::connection::InboundConnection;
use common::{Address, SmallString, SocketAddress};
use std::net::IpAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::prelude::*;

mod v5 {
    pub const VERSION: u8 = 5;
    pub const METH_NO_AUTH: u8 = 0;
    pub const CMD_CONNECT: u8 = 1;
    pub const TYPE_IPV4: u8 = 1;
    pub const TYPE_IPV6: u8 = 4;
    pub const TYPE_DOMAIN: u8 = 3;
}

fn other(desc: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, desc)
}

pub async fn serve(
    mut conn: InboundConnection<'_>,
) -> io::Result<(InboundConnection<'_>, SocketAddress)> {
    // Read version
    let version = conn.conn.read_u8().await?;
    if version != v5::VERSION {
        return Err(other("Incorrect version"));
    }

    // Read and drop methods
    let nmethods = conn.conn.read_u8().await?;
    conn.conn
        .read_exact(&mut vec![0; nmethods as usize])
        .await?;

    // METHOD selection message
    conn.conn.write(&[v5::VERSION, v5::METH_NO_AUTH]).await?;

    // Read request
    let addr_type = {
        let mut buffer = [0; 4]; // VER CMD RSV ATYP
        conn.conn.read_exact(&mut buffer).await?;
        if buffer[1] != v5::CMD_CONNECT {
            return Err(other("Unsupported command"));
        }
        buffer[3]
    };
    let address = match addr_type {
        v5::TYPE_IPV4 => {
            let mut buffer = [0; 4];
            conn.conn.read_exact(&mut buffer).await?;
            Address::Ip(IpAddr::from(buffer))
        }
        v5::TYPE_IPV6 => {
            let mut buffer = [0; 16];
            conn.conn.read_exact(&mut buffer).await?;
            Address::Ip(IpAddr::from(buffer))
        }
        v5::TYPE_DOMAIN => {
            let mut buffer = [0; 255];
            let len = conn.conn.read_u8().await? as usize;
            conn.conn.read_exact(&mut buffer[0..len]).await?;
            let s = String::from_utf8_lossy(&buffer[0..len]);
            Address::Domain(SmallString::from_str(&s))
        }
        _ => return Err(other("Invalid ATYP")),
    };
    let port = conn.conn.read_u16().await?;
    let socket_address = SocketAddress::new(address, port);

    // Send reply
    conn.conn
        .write(&[0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x08, 0x4])
        .await?;

    // And we are done
    return Ok((conn, socket_address));
}
