use crate::config::{Config, TransportType};
use crate::prelude::*;
use log::info;
use tokio::net::TcpStream;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::UnboundedReceiver;

type ConnReceiver<T> = UnboundedReceiver<(Connection, T)>;

pub async fn start_inbounds(
  config: &Config,
) -> Result<(
  ConnReceiver<TcpStream>,
  ConnReceiver<(Vec<u8>, Arc<UdpSocket>)>,
)> {
  use tokio::net::{TcpListener, UdpSocket};

  use tokio::sync::mpsc::unbounded_channel;

  let tcp_channel = unbounded_channel();
  let udp_channel = unbounded_channel();

  for inbound in &config.inbounds {
    let transport = &inbound.1.transport;
    let ip = transport.listen.unwrap_or([0, 0, 0, 0].into());
    let tag = inbound.0.clone();
    let pipe =inbound.1.pipeline.clone();
    let port = transport.port;

    match transport.r#type {
      TransportType::Tcp => {
        let listener = TcpListener::bind(&(ip, port)).await?;
        let sender = tcp_channel.0.clone();
        info!("Inbound {} listening on {}:{}", tag, ip, port);

        tokio::spawn(async move {
          loop {
            let (stream, src_addr) = listener.accept().await.unwrap();
            let conn = Connection::new(src_addr, tag.clone(), pipe.clone());
            info!("Inbound {} accepted from {}", tag, src_addr);
            sender.send((conn, stream)).unwrap();
          }
        });
      }
      TransportType::Udp => {
        unimplemented!();
      }
    };
  }

  Ok((tcp_channel.1, udp_channel.1))
}
