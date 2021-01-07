pub mod io;
pub mod metered_stream;
pub mod prepend_stream;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn unix_ts() -> Duration {
    let start = SystemTime::now();
    start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
}
