#![cfg(target_os = "android")]
use crate::prelude::*;
use anyhow::anyhow;

pub fn register(plumber: &mut Plumber) {
  plumber.register("android_nat", |_| Ok(Box::new(AndroidNatProcessor {})));
}

pub struct AndroidNatProcessor {}

impl AndroidNatProcessor {
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

#[async_trait]
impl Processor for AndroidNatProcessor {
  async fn process(
    self: Arc<Self>,
    stream: ProxyStream,
    conn: &mut Connection,
    ctx: AppContextRef,
  ) -> Result<ProxyStream> {
    self.process_conn(conn, &ctx)?;
    Ok(stream)
  }
}
