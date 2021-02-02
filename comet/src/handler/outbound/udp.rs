use super::{NewOutboundHandler, Outbound, OutboundHandler};
use crate::prelude::*;
use anyhow::anyhow;
use log::error;
use std::net::SocketAddr;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use tokio::sync::mpsc::channel;
use tokio_stream::wrappers::ReceiverStream;

pub struct UdpHandler {}

macro_rules! break_if_err {
    ($e:expr) => {
        match $e {
            Ok(x) => x,
            Err(e) => {
                error!("Failed to process UDP packet: {}", e);
                break;
            }
        }
    };
}

#[async_trait]
impl OutboundHandler for UdpHandler {
    async fn handle(
        &self,
        _tag: &str,
        conn: &mut Connection,
        ctx: &AppContextRef,
    ) -> Result<ProxyStream> {
        let resolved = self.resolve_addr(conn, ctx).await.ok();

        let addr_type = conn.get_var("addr_type").unwrap_or(&AddrType::V4);
        let bind_ip = match addr_type {
            AddrType::V4 => IpAddr::from(Ipv4Addr::from(0u32)),
            AddrType::V6 => IpAddr::from(Ipv6Addr::from(0u128)),
        };

        let socket = crate::net_wrapper::bind_udp(&SocketAddr::new(bind_ip, 0)).await?;

        let (read_sender, read_receiver) = channel::<UdpPacket>(10);
        let (write_sender, mut write_receiver) = channel::<UdpPacket>(10);

        tokio::spawn(async move {
            loop {
                let mut buffer = [0u8; 4096];
                tokio::select! {
                    recv_res = socket.recv_from(&mut buffer) => {
                        let (n, addr) = break_if_err!(recv_res);
                        let packet = BytesMut::from(&buffer[0..n]);
                        if let Err(_) = read_sender.send(UdpPacket::new(addr, packet)).await {
                            // Dropped
                            break;
                        }
                    }
                    Some(packet) = write_receiver.recv() => {
                        if let Some((ref ips, port)) = resolved {
                            break_if_err!(socket.send_to(&packet, &SocketAddr::new(ips[0], port)).await);
                        } else if let Some(target) = packet.target() {
                            break_if_err!(socket.send_to(&packet, target).await);
                        } else {
                            break_if_err!(Err(anyhow!("No valid address specified")));
                        }
                    }
                    _ = read_sender.closed() => break,
                    else => break
                }
            }
        });

        Ok(UdpStream::new(ReceiverStream::new(read_receiver), write_sender).into())
    }
}

impl NewOutboundHandler for UdpHandler {
    fn new(_config: &Outbound) -> Self {
        Self {}
    }
}
