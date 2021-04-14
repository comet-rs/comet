use crate::prelude::*;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::net::SocketAddr;
use tokio::net::{TcpSocket, TcpStream, UdpSocket};

#[cfg(target_os = "android")]
mod protect;

pub async fn connect_tcp(addr: SocketAddr) -> IoResult<TcpStream> {
    let sock = match addr {
        SocketAddr::V4(_) => TcpSocket::new_v4(),
        SocketAddr::V6(_) => TcpSocket::new_v6(),
    }?;

    #[cfg(target_os = "android")]
    {
        use std::os::unix::io::AsRawFd;
        let fd = sock.as_raw_fd();
        protect::protect_async(fd).await?;
    }
    sock.connect(addr).await
}

pub async fn bind_udp(addr: SocketAddr) -> IoResult<UdpSocket> {
    let domain = Domain::for_address(addr);
    let sock = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
    sock.set_nonblocking(true)?;

    #[cfg(target_os = "android")]
    {
        use std::os::unix::io::AsRawFd;
        let fd = sock.as_raw_fd();
        protect::protect_async(fd).await?;
    }
    sock.bind(&SockAddr::from(addr))?;
    UdpSocket::from_std(sock.into())
}
