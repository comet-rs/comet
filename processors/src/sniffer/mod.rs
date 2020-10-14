mod http;
mod tls;

use bytes::{BufMut, BytesMut};
use common::*;
use log::warn;
use std::str;
use tokio::prelude::*;

#[derive(Debug)]
pub enum SniffStatus {
    NoClue,
    Fail(&'static str),
    Success(String),
}

pub async fn sniff(mut stream: RWPair, conn: &mut Connection) -> std::io::Result<RWPair> {
    let mut buffer = BytesMut::with_capacity(1024);

    let mut attempts: u8 = 0;
    let mut http_failed = false;
    let mut tls_failed = false;
    let dest_port = conn.dest_addr.as_ref().unwrap().port;

    while attempts < 5 && buffer.remaining_mut() > 0 {
        let read_bytes = stream.read_buf(&mut buffer).await?;
        if read_bytes == 0 {
            warn!("Got EOF while sniffing: {:?}", buffer);
            return Ok(stream.prepend_data(buffer));
        }

        if !http_failed {
            match http::sniff(&buffer) {
                SniffStatus::NoClue => (),
                SniffStatus::Fail(_) => {
                    http_failed = true;
                }
                SniffStatus::Success(s) => {
                    conn.dest_addr = Some(SocketAddress::new_domain(s, dest_port));
                    return Ok(stream.prepend_data(buffer));
                }
            }
        }

        if !tls_failed {
            match tls::sniff(&buffer) {
                SniffStatus::NoClue => (),
                SniffStatus::Fail(_) => {
                    tls_failed = true;
                }
                SniffStatus::Success(s) => {
                    conn.dest_addr = Some(SocketAddress::new_domain(s, dest_port));
                    return Ok(stream.prepend_data(buffer));
                }
            }
        }
        if http_failed && tls_failed {
            break;
        }
        attempts += 1;
    }

    return Ok(stream.prepend_data(buffer));
}
