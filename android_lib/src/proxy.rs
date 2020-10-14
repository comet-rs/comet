use crate::nat_manager::{NatManagerRef, ProtocolType};
use crate::IPV4_CLIENT;
use anyhow::{Context, Result};
use log::{error, info};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct ProxyPorts {
    pub tcp_v4: u16,
    pub udp_v4: u16,
    pub dns_v4: u16,
    pub tcp_v6: u16,
    pub udp_v6: u16,
    pub dns_v6: u16,
}

impl ProxyPorts {
    pub fn new() -> Self {
        ProxyPorts {
            tcp_v4: 0,
            udp_v4: 0,
            dns_v4: 0,
            tcp_v6: 0,
            udp_v6: 0,
            dns_v6: 0,
        }
    }
}

async fn listen_tcp(ports: &mut ProxyPorts) -> Result<(TcpListener,)> {
    let listener_v4 = TcpListener::bind((IPV4_CLIENT, 0)).await?;
    ports.tcp_v4 = listener_v4.local_addr()?.port();
    info!("TCP proxy listening on port {} (v4)", ports.tcp_v4);
    Ok((listener_v4,))
}

async fn listen_udp(ports: &mut ProxyPorts) -> Result<(UdpSocket,)> {
    let socket_v4 = UdpSocket::bind(SocketAddr::new(IpAddr::V4(IPV4_CLIENT), 0)).await?;
    ports.udp_v4 = socket_v4.local_addr()?.port();
    info!("UDP proxy listening on port {} (v4)", ports.udp_v4);
    Ok((socket_v4,))
}

async fn listen_dns(ports: &mut ProxyPorts) -> Result<(UdpSocket,)> {
    let socket_v4 = UdpSocket::bind(SocketAddr::new(IpAddr::V4(IPV4_CLIENT), 0)).await?;
    ports.dns_v4 = socket_v4.local_addr()?.port();
    info!("DNS proxy listening on port {} (v4)", ports.dns_v4);
    Ok((socket_v4,))
}

async fn process_socket(
    socket: TcpStream,
    src_addr: SocketAddr,
    dest_addr: SocketAddr,
) -> Result<()> {
    use common::*;
    use transport::outbound::{OutboundTcpTransport, OutboundTransport};

    let mut socket = RWPair::new(socket);

    let mut conn = Connection::new(src_addr);
    conn.dest_addr = Some(dest_addr.into());

    socket = processors::sniffer::sniff(socket, &mut conn).await?;

    info!("Conn: {:?}", conn);

    let mut out_conn = match conn.dest_addr.as_ref().unwrap().addr {
        Address::Domain(_) => {
            let transport = OutboundTcpTransport
                .connect(SocketAddr::new([192, 168, 1, 106].into(), 8888))
                .await?;

            processors::http_proxy::client_handshake(transport, &mut conn).await?
        }
        Address::Ip(_) => OutboundTcpTransport.connect(dest_addr).await?,
    };

    let copy_count = socket.bidi_copy(&mut out_conn).await?;

    info!("C -> S: {}, S -> C: {}", copy_count.0, copy_count.1);
    Ok(())
}

async fn run_tcp(listeners: (TcpListener,), manager: NatManagerRef) -> Result<()> {
    let mut listener_v4 = listeners.0;
    loop {
        let (socket, src_addr) = listener_v4
            .accept()
            .await
            .with_context(|| "Failed to accept")?;
        let entry = manager.get_entry(ProtocolType::Tcp, src_addr.port(), src_addr.ip());

        if let Some((dest_addr, dest_port)) = entry {
            info!(
                "TCP: New socket: {:?} (real {:?}:{})",
                src_addr, dest_addr, dest_port
            );
            tokio::spawn(async move {
                let result =
                    process_socket(socket, src_addr, SocketAddr::new(dest_addr, dest_port)).await;
                match result {
                    Err(error) => {
                        error!("Error while processing: {:?}", error);
                    }
                    Ok(_) => {
                        info!(
                            "Successfully processed connection to {:?}:{}",
                            dest_addr, dest_port
                        );
                    }
                }
            });
        } else {
            error!("New socket: {:?} (invalid, dropping)", src_addr);
        }
    }
}

async fn run_udp(_sockets: (UdpSocket,), _manager: NatManagerRef) -> Result<()> {
    Ok(())
}

async fn run_dns(sockets: (UdpSocket,), _manager: NatManagerRef) -> Result<()> {
    let socket_v4 = Arc::new(Mutex::new(sockets.0));

    loop {
        let mut buffer = [0u8; 512];
        let (size, src_addr) = socket_v4.lock().await.recv_from(&mut buffer).await?;

        let cloned_socket = Arc::clone(&socket_v4);
        tokio::spawn(async move {
            let result = dns::process_query(&buffer[0..size]).await;
            match result {
                Ok(packet) => {
                    let _ = cloned_socket
                        .lock()
                        .await
                        .send_to(&packet[..], src_addr)
                        .await;
                }
                Err(error) => {
                    error!("Error while resolving: {:?}", error);
                }
            }
        });
    }
}

pub async fn start_proxy(manager: NatManagerRef) -> Result<ProxyPorts> {
    let mut ports = ProxyPorts::new();
    let tcp_listeners = listen_tcp(&mut ports).await?;
    let udp_listeners = listen_udp(&mut ports).await?;
    let dns_listeners = listen_dns(&mut ports).await?;

    let manager_tcp = Arc::clone(&manager);
    tokio::spawn(async move {
        let exit_status = run_tcp(tcp_listeners, manager_tcp).await;

        if let Err(error) = exit_status {
            error!("TCP proxy thread exited: {:?}", error);
        }
    });

    let manager_udp = Arc::clone(&manager);
    tokio::spawn(async move {
        let exit_status = run_udp(udp_listeners, manager_udp).await;

        if let Err(error) = exit_status {
            error!("UDP proxy thread exited: {:?}", error);
        }
    });

    let manager_dns = Arc::clone(&manager);
    tokio::spawn(async move {
        let exit_status = run_dns(dns_listeners, manager_dns).await;

        if let Err(error) = exit_status {
            error!("DNS proxy thread exited: {:?}", error);
        }
    });
    Ok(ports)
}
