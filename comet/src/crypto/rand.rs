use crate::prelude::*;

use xorshift::{thread_rng, xoroshiro128::Xoroshiro128};

pub fn rand_bytes(buf: &mut [u8]) -> Result<()> {
  #[cfg(target_os = "windows")]
  {
    use win_crypto_ng::random::*;
    let rng = RandomNumberGenerator::system_preferred();
    return Ok(rng.gen_random(buf)?);
  }

  #[cfg(not(target_os = "windows"))]
  {
    use openssl::rand::rand_bytes as openssl_rand_bytes;
    return Ok(openssl_rand_bytes(buf)?);
  }
}

pub fn xor_rng() -> Xoroshiro128 {
  thread_rng()
}
