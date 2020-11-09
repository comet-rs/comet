use crate::config::{Config, Outbound, OutboundAddr, OutboundTransportType};
use crate::prelude::*;
use crate::utils::metered_stream::MeteredStream;
use anyhow::anyhow;
use std::collections::HashMap;
use std::net::IpAddr;
use std::net::SocketAddr;
use tokio::io::BufReader;
use tokio::sync::mpsc::channel;

pub struct OutboundManager {
  outbounds: HashMap<SmolStr, Outbound>,
}

impl OutboundManager {
  pub fn new(config: &Config) -> Self {
    Self {
      outbounds: config.outbounds.clone(),
    }
  }

  pub async fn connect(
    &self,
    tag: &str,
    conn: &mut Connection,
    ctx: &AppContextRef,
  ) -> Result<ProxyStream> {
    let outbound = self.get_outbound(tag)?;
    Ok(match outbound.transport.r#type {
      OutboundTransportType::Tcp => self.connect_tcp_multi(tag, conn, ctx).await?.into(),
      OutboundTransportType::Udp => self.connect_udp(tag, conn, ctx).await?.into(),
    })
  }

  async fn connect_tcp(
    &self,
    tag: &str,
    addr: IpAddr,
    port: u16,
    ctx: &AppContextRef,
  ) -> Result<RWPair> {
    let outbound = self.get_outbound(tag)?;

    let port = outbound.transport.port.unwrap_or(port);

    let stream = crate::net_wrapper::connect_tcp(&SocketAddr::from((addr, port))).await?;
    Ok(if outbound.metering {
      RWPair::new(MeteredStream::new_outbound(
        BufReader::new(stream),
        &tag,
        &ctx,
      ))
    } else {
      RWPair::new(BufReader::new(stream))
    })
  }

  pub async fn connect_tcp_multi(
    &self,
    tag: &str,
    conn: &mut Connection,
    ctx: &AppContextRef,
  ) -> Result<RWPair> {
    let outbound = self.get_outbound(tag)?;
    let port = if let Some(port) = outbound.transport.port {
      port
    } else {
      conn.dest_addr.port_or_error()?
    };

    let ips = if let Some(addr) = &outbound.transport.addr {
      // Dest addr overridden
      match addr {
        OutboundAddr::Ip(ip) => vec![*ip],
        OutboundAddr::Domain(domain) => ctx.dns.resolve(&domain).await?,
      }
    } else {
      ctx.dns.resolve_addr(&conn.dest_addr).await?
    };

    for ip in ips {
      match self.connect_tcp(tag, ip, port, ctx).await {
        Ok(stream) => return Ok(stream),
        Err(err) => error!("Trying {}:{} failed: {}", ip, port, err),
      }
    }
    Err(anyhow!("All attempts failed"))
  }

  pub async fn connect_udp(
    &self,
    tag: &str,
    conn: &Connection,
    ctx: &AppContextRef,
  ) -> Result<UdpStream> {
    let outbound = self.get_outbound(tag)?;
    let port = if let Some(port) = outbound.transport.port {
      port
    } else {
      conn.dest_addr.port_or_error()?
    };

    let ips = if let Some(addr) = &outbound.transport.addr {
      // Dest addr overridden
      match addr {
        OutboundAddr::Ip(ip) => vec![*ip],
        OutboundAddr::Domain(domain) => ctx.dns.resolve(&domain).await?,
      }
    } else {
      ctx.dns.resolve_addr(&conn.dest_addr).await?
    };

    let socket = Arc::new(
      crate::net_wrapper::bind_udp(&SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 0)).await?,
    );
    socket.connect(&SocketAddr::new(ips[0], port)).await?;

    let (read_sender, read_receiver) = channel::<BytesMut>(10);
    let (write_sender, mut write_receiver) = channel::<BytesMut>(10);

    let read_sender_clone = read_sender.clone();
    let socket_clone = socket.clone();
    tokio::spawn(async move {
      loop {
        let mut buffer = [0u8; 4096];
        tokio::select! {
          Ok(n) = socket_clone.recv(&mut buffer) => {
            let packet = BytesMut::from(&buffer[0..n]);
            read_sender_clone.send(packet).await.unwrap();
          }
          Some(packet) = write_receiver.recv() => {
            socket_clone.send(&packet).await.unwrap();
          }
          _ = read_sender_clone.closed() => break,
          else => break
        }
      }
    });

    Ok(UdpStream::new(read_receiver, write_sender))
  }

  pub fn get_pipeline(&self, tag: &str) -> Result<Option<&str>> {
    Ok(match self.get_outbound(tag)?.pipeline.as_ref() {
      Some(r) => Some(r),
      None => None,
    })
  }

  pub fn get_outbound(&self, tag: &str) -> Result<&Outbound> {
    let outbound = self
      .outbounds
      .get(tag)
      .ok_or_else(|| anyhow!("Outbound {} not found", tag))?;
    Ok(outbound)
  }
}
