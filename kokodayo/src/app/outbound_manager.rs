use crate::config::{Config, Outbound};
use crate::prelude::*;
use crate::utils::metered_stream::MeteredReader;
use crate::utils::metered_stream::MeteredWriter;
use crate::utils::unix_ts;
use anyhow::anyhow;
use std::collections::HashMap;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::BufReader;
use tokio::net::UdpSocket;

struct UdpSocketEntry {
  dest: SocketAddr,
  socket: Arc<UdpSocket>,
  last_active: AtomicU64,
}

impl UdpSocketEntry {
  fn new(dest: SocketAddr, socket: Arc<UdpSocket>) -> Self {
    Self {
      dest,
      socket,
      last_active: AtomicU64::new(unix_ts().as_secs()),
    }
  }

  fn refresh(&self) -> bool {
    let now = unix_ts().as_secs();
    let last = self.last_active.swap(now, Ordering::Relaxed);
    now - last < 3600
  }

  fn clone_socket(&self) -> Arc<UdpSocket> {
    self.socket.clone()
  }
}

pub struct OutboundManager {
  outbounds: HashMap<SmolStr, Outbound>,
  udp_sockets: flurry::HashMap<SocketAddr, UdpSocketEntry>,
}

impl OutboundManager {
  pub fn new(config: &Config) -> Self {
    Self {
      outbounds: config.outbounds.clone(),
      udp_sockets: flurry::HashMap::new(),
    }
  }

  pub async fn connect_tcp(
    &self,
    tag: &str,
    addr: IpAddr,
    port: u16,
    ctx: &AppContextRef,
  ) -> Result<RWPair> {
    let outbound = self.outbounds.get(tag).unwrap();
    assert!(outbound.transport.r#type == TransportType::Tcp);

    let port = outbound.transport.port.unwrap_or(port);
    let addr = outbound.transport.addr.unwrap_or(addr);

    let stream = crate::net_wrapper::connect_tcp(&SocketAddr::from((addr, port))).await?;
    let splitted = stream.into_split();

    Ok(RWPair::new_parts(
      BufReader::new(MeteredReader::new_outbound(splitted.0, &tag, ctx)),
      MeteredWriter::new_outbound(splitted.1, &tag, ctx),
    ))
  }

  pub async fn connect_tcp_multi(
    &self,
    tag: &str,
    addrs: Vec<IpAddr>,
    port: u16,
    ctx: &AppContextRef,
  ) -> Result<RWPair> {
    for addr in addrs {
      match self.connect_tcp(tag, addr, port, ctx).await {
        Ok(stream) => return Ok(stream),
        Err(err) => error!("Trying {}:{} failed: {}", addr, port, err),
      }
    }
    Err(anyhow!("All attempts failed"))
  }

  pub async fn connect_udp(
    &self,
    tag: &str,
    conn: &Connection,
    dest_addr: SocketAddr,
    _ctx: &AppContextRef,
  ) -> Result<Arc<UdpSocket>> {
    let outbound = self.outbounds.get(tag).unwrap();
    assert!(outbound.transport.r#type == TransportType::Udp);

    if let Some(entry) = self.udp_sockets.pin().get(&conn.src_addr) {
      if entry.refresh() && dest_addr == entry.dest {
        warn!("Reusing UDP socket {} -> {}", conn.src_addr, dest_addr);
        return Ok(entry.clone_socket());
      }
    }

    let socket = Arc::new(
      crate::net_wrapper::bind_udp(&SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 0)).await?,
    );
    socket.connect(&dest_addr).await?;

    self.udp_sockets.pin().insert(
      conn.src_addr,
      UdpSocketEntry::new(dest_addr, socket.clone()),
    );

    Ok(socket)
  }

  pub fn get_pipeline(&self, tag: &str, transport_type: TransportType) -> Option<&str> {
    let outbound = self.outbounds.get(tag).unwrap();
    assert!(outbound.transport.r#type == transport_type);

    match outbound.pipeline.as_ref() {
      Some(r) => Some(r),
      None => None,
    }
  }
}
