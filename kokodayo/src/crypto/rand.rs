use crate::prelude::*;
use openssl::rand::rand_bytes as openssl_rand_bytes;
use xorshift::{thread_rng, xoroshiro128::Xoroshiro128};

pub fn rand_bytes(buf: &mut [u8]) -> Result<()> {
  Ok(openssl_rand_bytes(buf)?)
}

pub fn xor_rng() -> Xoroshiro128 {
  thread_rng()
}
