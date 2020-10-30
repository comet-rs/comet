use crate::prelude::*;
use openssl::rand::rand_bytes as openssl_rand_bytes;

pub fn rand_bytes(buf: &mut [u8]) -> Result<()> {
  Ok(openssl_rand_bytes(buf)?)
}