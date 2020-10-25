use crate::config::{Config, Inbound};
use crate::prelude::*;
use crate::utils::metered_stream::{MeteredReader, MeteredWriter};
use log::info;
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use tokio::io::BufReader;
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

pub type ConnSender<T> = UnboundedSender<(Connection, T)>;
pub type ConnReceiver<T> = UnboundedReceiver<(Connection, T)>;
pub type TcpConnSender = ConnSender<RWPair>;

pub struct InboundManager {
  inbounds: HashMap<SmolStr, Inbound>,
  tcp_sender: OnceCell<ConnSender<RWPair>>,
  udp_sender: OnceCell<ConnSender<UdpRequest>>,
}

impl InboundManager {
  pub fn new(config: &Config) -> Self {
    InboundManager {
      inbounds: config.inbounds.clone(),
      tcp_sender: OnceCell::new(),
      udp_sender: OnceCell::new(),
    }
  }

  pub async fn start(
    self: Arc<Self>,
    ctx: AppContextRef,
  ) -> Result<(ConnReceiver<RWPair>, ConnReceiver<UdpRequest>)> {
    let tcp_channel = unbounded_channel();
    let udp_channel = unbounded_channel();

    for inbound in &self.inbounds {
      let ctx = ctx.clone();
      let transport = &inbound.1.transport;
      let ip = transport.listen.unwrap_or_else(|| [0, 0, 0, 0].into());
      let tag = inbound.0.clone();
      let pipe = inbound.1.pipeline.clone();
      let port = transport.port;

      match transport.r#type {
        TransportType::Tcp => {
          let listener = TcpListener::bind(&(ip, port)).await?;
          let sender = tcp_channel.0.clone();
          info!("Inbound {}/TCP listening on {}:{}", tag, ip, port);

          tokio::spawn(async move {
            loop {
              let (stream, src_addr) = listener.accept().await.unwrap();
              let conn = Connection::new(src_addr, tag.clone(), pipe.clone(), TransportType::Tcp);
              let splitted = stream.into_split();
              info!("Inbound {}/TCP accepted from {}", tag, src_addr);
              sender
                .send((
                  conn,
                  RWPair::new_parts(
                    BufReader::new(MeteredReader::new_inbound(splitted.0, &tag, &ctx)),
                    MeteredWriter::new_inbound(splitted.1, &tag, &ctx),
                  ),
                ))
                .unwrap();
            }
          });
        }
        TransportType::Udp => {
          let socket = Arc::new(UdpSocket::bind(&(ip, port)).await?);
          let sender = udp_channel.0.clone();
          info!("Inbound {}/UDP listening on {}:{}", tag, ip, port);

          tokio::spawn(async move {
            loop {
              let mut buffer = [0u8; 4096];
              let (size, src_addr) = socket.recv_from(&mut buffer).await.unwrap();
              let packet = buffer[0..size].to_vec();
              let conn = Connection::new(src_addr, tag.clone(), pipe.clone(), TransportType::Udp);
              info!("Inbound {}/UDP accepted from {}", tag, src_addr);

              sender
                .send((conn, UdpRequest::new(socket.clone(), packet)))
                .unwrap();
            }
          });
        }
      };
    }

    self.tcp_sender.set(tcp_channel.0.clone()).unwrap();
    self.udp_sender.set(udp_channel.0.clone()).unwrap();
    Ok((tcp_channel.1, udp_channel.1))
  }

  pub fn inject_tcp(&self) {}
  pub fn inject_udp(&self) {}
}
