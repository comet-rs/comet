pub mod io;
pub mod metered_stream;
pub mod connector;

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use anyhow::Result;

pub fn unix_ts() -> Duration {
    let start = SystemTime::now();
    start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
}

pub fn urlsafe_base64_decode_string<T: AsRef<[u8]>>(input: T) -> Result<String> {
    let bytes = base64::decode_config(input, base64::URL_SAFE)?;
    let ret = String::from_utf8(bytes)?;
    Ok(ret)
}