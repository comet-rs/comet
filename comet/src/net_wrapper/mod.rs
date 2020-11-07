use net2::UdpBuilder;
use std::io;
use std::net::SocketAddr;
use tokio::net::{TcpSocket, TcpStream, UdpSocket};

#[cfg(target_os = "android")]
mod protect;

pub async fn connect_tcp(addr: &SocketAddr) -> io::Result<TcpStream> {
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
    sock.connect(*addr).await
}

pub async fn bind_udp(addr: &SocketAddr) -> io::Result<UdpSocket> {
    let sock = match addr {
        SocketAddr::V4(_) => UdpBuilder::new_v4(),
        SocketAddr::V6(_) => UdpBuilder::new_v6(),
    }?;

    #[cfg(target_os = "android")]
    {
        use std::os::unix::io::AsRawFd;
        let fd = sock.as_raw_fd();
        protect::protect_async(fd).await?;
    }
    let s = sock.bind(&addr)?;
    UdpSocket::from_std(s)
}
