mod http;
mod tls;

use bytes::{BufMut, BytesMut};
use common::Address;
use log::info;
use std::str;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;

#[derive(Debug)]
pub enum SniffStatus {
    NoClue,
    Fail(&'static str),
    Success(String),
}

pub async fn sniff<R: AsyncRead + Unpin>(
    conn: &mut R,
) -> std::io::Result<(BytesMut, Option<Address>)> {
    let mut buffer = BytesMut::with_capacity(300);

    let mut attempts = 0;
    let mut http_failed = false;
    let mut tls_failed = false;

    while attempts < 3 && buffer.remaining_mut() > 0 {
        let read_bytes = conn.read_buf(&mut buffer).await?;
        if read_bytes == 0 {
            // EOF
            return Ok((buffer, None));
        }

        if !http_failed {
            match http::sniff(&buffer) {
                SniffStatus::NoClue => (),
                SniffStatus::Fail(reason) => {
                    http_failed = true;
                    info!("HTTP sniffing failed: {}", reason);
                }
                SniffStatus::Success(s) => return Ok((buffer, Some(Address::Domain(s.into())))),
            }
        }
        if !tls_failed {
            match tls::sniff(&buffer) {
                SniffStatus::NoClue => (),
                SniffStatus::Fail(reason) => {
                    tls_failed = true;
                    info!("TLS sniffing failed: {}", reason);
                }
                SniffStatus::Success(s) => return Ok((buffer, Some(Address::Domain(s.into())))),
            }
        }
        if http_failed && tls_failed {
            break;
        }
        attempts += 1;
    }

    Ok((buffer, None))
}
