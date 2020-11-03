#![cfg(target_os = "android")]
use crate::app::plumber::UdpProcessor;
use crate::prelude::*;
use anyhow::anyhow;

pub struct AndroidNatProcessor {}

impl AndroidNatProcessor {
  pub fn new(_config: &AndroidNatConfig) -> Result<Self> {
    Ok(AndroidNatProcessor {})
  }

  pub fn process_conn(&self, conn: &mut Connection, ctx: &AppContextRef) -> Result<()> {
    let manager = &ctx.nat_manager;
    let entry = manager.get_entry(conn.typ, conn.src_addr.port(), conn.src_addr.ip());

    match entry {
      Some((dest_addr, dest_port)) => {
        conn.dest_addr.set_ip(dest_addr);
        conn.dest_addr.set_port(dest_port);
        Ok(())
      }
      None => Err(anyhow!("Entry not found in NAT table")),
    }
  }
}

#[derive(Clone, Debug, Deserialize)]
pub struct AndroidNatConfig {}

#[async_trait]
impl Processor for AndroidNatProcessor {
  async fn process(
    self: Arc<Self>,
    stream: RWPair,
    conn: &mut Connection,
    ctx: AppContextRef,
  ) -> Result<RWPair> {
    self.process_conn(conn, &ctx)?;
    Ok(stream)
  }
}

#[async_trait]
impl UdpProcessor for AndroidNatProcessor {
  async fn process_udp(
    self: Arc<Self>,
    req: UdpRequest,
    conn: &mut Connection,
    ctx: AppContextRef,
  ) -> Result<UdpRequest> {
    self.process_conn(conn, &ctx)?;
    Ok(req)
  }
}
