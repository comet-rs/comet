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
  udp_sender: OnceCell<ConnSender<(Vec<u8>, Arc<UdpSocket>)>>,
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
  ) -> Result<(
    ConnReceiver<RWPair>,
    ConnReceiver<(Vec<u8>, Arc<UdpSocket>)>,
  )> {
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
          info!("Inbound {} listening on {}:{}", tag, ip, port);

          tokio::spawn(async move {
            loop {
              let (stream, src_addr) = listener.accept().await.unwrap();
              let conn = Connection::new(src_addr, tag.clone(), pipe.clone(), TransportType::Tcp);
              let splitted = stream.into_split();
              info!("Inbound {} accepted from {}", tag, src_addr);
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
          unimplemented!();
        }
      };
    }

    self.tcp_sender.set(tcp_channel.0.clone()).unwrap();
    self.udp_sender.set(udp_channel.0.clone()).unwrap();
    Ok((tcp_channel.1, udp_channel.1))
  }
}
