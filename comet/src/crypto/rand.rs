use crate::prelude::*;

use rand::rngs::OsRng;
use rand::RngCore;
use xorshift::{thread_rng, xoroshiro128::Xoroshiro128};

pub fn rand_bytes(buf: &mut [u8]) -> Result<()> {
    OsRng.fill_bytes(buf);
    Ok(())
}

pub fn xor_rng() -> Xoroshiro128 {
    thread_rng()
}
