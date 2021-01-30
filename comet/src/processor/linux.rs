use crate::prelude::*;
use anyhow::anyhow;

pub fn register(plumber: &mut Plumber) {
    plumber.register("associate_uid", |_, _| {
        Ok(Box::new(AssociateUidProcessor {}))
    });
}

pub struct AssociateUidProcessor {}

fn find_uid(content: &str, port: u16) -> Result<Option<u32>> {
    let mut lines = content.lines().map(|l| l.trim());
    let heading = lines
        .next()
        .ok_or_else(|| anyhow!("Unable to read heading"))?;
    let uid_pos = heading
        .split_ascii_whitespace()
        .position(|s| s == "uid")
        .ok_or_else(|| anyhow!("Unable to position of column 'uid'"))?;

    while let Some(line) = lines.next_back() {
        let mut split = line.split_ascii_whitespace();
        split.next(); // Skip sl
        let local_port = split
            .next()
            .and_then(|s| u16::from_str_radix(&s[s.len() - 4..], 16).ok())
            .ok_or_else(|| anyhow!("Unable to parse local port"))?;

        if local_port == port {
            let uid = split
                .nth(uid_pos - 4)
                .ok_or_else(|| anyhow!("Unable to parse uid"))?;
            return Ok(Some(u32::from_str_radix(uid, 10)?));
        }
    }
    Ok(None)
}

impl AssociateUidProcessor {
    pub async fn process_conn(&self, conn: &mut Connection, _ctx: &AppContextRef) -> Result<()> {
        match conn.typ {
            TransportType::Tcp => {
                // Android seems to assign IPv4 connections to `tcp6`, wtf?
                for path in &["/proc/net/tcp6", "/proc/net/tcp"] {
                    let content = tokio::fs::read_to_string(&path).await?;
                    if let Some(uid) = find_uid(&content, conn.src_addr.port())? {
                        conn.set_var("unix_uid", uid);
                        break;
                    }
                }
            }
            TransportType::Udp => {
                for path in &["/proc/net/udp", "/proc/net/udp6"] {
                    let content = tokio::fs::read_to_string(&path).await?;
                    if let Some(uid) = find_uid(&content, conn.src_addr.port())? {
                        conn.set_var("unix_uid", uid);
                        break;
                    }
                }
            }
        };
        Ok(())
    }
}

#[async_trait]
impl Processor for AssociateUidProcessor {
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        conn: &mut Connection,
        ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        self.process_conn(conn, &ctx).await?;
        Ok(stream)
    }
}
