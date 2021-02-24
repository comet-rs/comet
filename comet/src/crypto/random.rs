use crate::prelude::*;

use rand::{rngs::OsRng, Rng, SeedableRng};
use rand::{thread_rng, RngCore};
use rand_xorshift::XorShiftRng;

pub fn rand_bytes(buf: &mut [u8]) -> Result<()> {
    OsRng.fill_bytes(buf);
    Ok(())
}

pub fn xor_rng() -> XorShiftRng {
    let mut seed = [0u8; 16];
    thread_rng().fill(&mut seed[..]);
    XorShiftRng::from_seed(seed)
}
