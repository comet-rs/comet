mod http;
mod tls;

use bytes::{BufMut, BytesMut};
use common::Address;
use log::warn;
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
    let mut buffer = BytesMut::with_capacity(1024);

    let mut attempts: u8 = 0;
    let mut http_failed = false;
    let mut tls_failed = false;

    while attempts < 5 && buffer.remaining_mut() > 0 {
        let read_bytes = conn.read_buf(&mut buffer).await?;
        if read_bytes == 0 {
            warn!("Got EOF while sniffing: {:?}", buffer);
            return Ok((buffer, None));
        }

        if !http_failed {
            match http::sniff(&buffer) {
                SniffStatus::NoClue => (),
                SniffStatus::Fail(_) => {
                    http_failed = true;
                }
                SniffStatus::Success(s) => {
                    return Ok((buffer, Some(Address::Domain(s.into()))));
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
                    return Ok((buffer, Some(Address::Domain(s.into()))));
                }
            }
        }
        if http_failed && tls_failed {
            break;
        }
        attempts += 1;
    }

    Ok((buffer, None))
}
