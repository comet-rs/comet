pub mod metered_stream;
pub mod io;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn unix_ts() -> Duration {
  let start = SystemTime::now();
  start
    .duration_since(UNIX_EPOCH)
    .expect("Time went backwards")
}
