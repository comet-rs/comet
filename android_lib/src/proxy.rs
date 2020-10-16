use crate::nat_manager::{NatManagerRef, ProtocolType};
use crate::VpnListeners;
use anyhow::{Context, Result};
use log::{error, info};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::Mutex;

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

pub async fn start_proxy(manager: NatManagerRef, listeners: VpnListeners) -> Result<()> {
    let tcp = listeners.tcp_v4;
    let dns = listeners.udp_v4;

    let manager_tcp = Arc::clone(&manager);
    tokio::spawn(async move {
        let exit_status = run_tcp((tcp,), manager_tcp).await;
        if let Err(error) = exit_status {
            error!("TCP proxy thread exited: {:?}", error);
        }
    });

    // let manager_udp = Arc::clone(&manager);
    // tokio::spawn(async move {
    //     let exit_status = run_udp((listeners.udp_v4,), manager_udp).await;

    //     if let Err(error) = exit_status {
    //         error!("UDP proxy thread exited: {:?}", error);
    //     }
    // });

    let manager_dns = Arc::clone(&manager);
    tokio::spawn(async move {
        let exit_status = run_dns((dns,), manager_dns).await;
        if let Err(error) = exit_status {
            error!("DNS proxy thread exited: {:?}", error);
        }
    });
    Ok(())
}
