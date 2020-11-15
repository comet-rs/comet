use super::{NewOutboundHandler, Outbound, OutboundAddr, OutboundHandler};
use crate::config::OutboundTransportConfig;
use crate::prelude::*;
use std::net::IpAddr;
use std::net::SocketAddr;
use tokio::sync::mpsc::channel;

pub struct UdpHandler {
  transport: OutboundTransportConfig,
}

#[async_trait]
impl OutboundHandler for UdpHandler {
  async fn handle(
    &self,
    _tag: &str,
    conn: &mut Connection,
    ctx: &AppContextRef,
  ) -> Result<ProxyStream> {
    let (ips, port) = self.resolve_addr(conn, ctx).await?;

    let socket = Arc::new(
      crate::net_wrapper::bind_udp(&SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 0)).await?,
    );
    socket.connect(&SocketAddr::new(ips[0], port)).await?;

    let (read_sender, read_receiver) = channel::<BytesMut>(10);
    let (write_sender, mut write_receiver) = channel::<BytesMut>(10);

    let socket_clone = socket.clone();
    tokio::spawn(async move {
      loop {
        let mut buffer = [0u8; 4096];
        tokio::select! {
          Ok(n) = socket_clone.recv(&mut buffer) => {
            let packet = BytesMut::from(&buffer[0..n]);
            read_sender.send(packet).await.unwrap();
          }
          Some(packet) = write_receiver.recv() => {
            socket_clone.send(&packet).await.unwrap();
          }
          _ = read_sender.closed() => break,
          else => break
        }
      }
    });

    Ok(UdpStream::new(read_receiver, write_sender).into())
  }
  fn port(&self) -> std::option::Option<u16> {
    self.transport.port
  }
  fn addr(&self) -> std::option::Option<&OutboundAddr> {
    self.transport.addr.as_ref()
  }
}

impl NewOutboundHandler for UdpHandler {
  fn new(config: &Outbound) -> Self {
    Self {
      transport: config.transport.clone(),
    }
  }
}
