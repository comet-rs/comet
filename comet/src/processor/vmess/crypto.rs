use sha3::{
    digest::{ExtendableOutputDirty, Update, XofReader},
    Sha3XofReader, Shake128,
};

use crate::crypto::{
    aead::NonceSeq,
    hashing::{hash_bytes, HashKind},
};

pub struct ShakeGenerator(Sha3XofReader);

impl ShakeGenerator {
    pub fn new(nonce: &[u8]) -> Self {
        let mut shake = Shake128::default();
        shake.update(nonce);

        let shake_reader = shake.finalize_xof_dirty();
        Self(shake_reader)
    }

    fn next(&mut self) -> u16 {
        let mut buf = [0u8; 2];
        self.0.read(&mut buf);

        u16::from_be_bytes(buf)
    }

    pub fn encode(&mut self, len: u16) -> u16 {
        self.next() ^ len
    }

    pub fn next_padding(&mut self) -> usize {
        (self.next() % 64) as usize
    }

    pub fn max_padding(&self) -> usize {
        64
    }
}

pub struct VmessNonceSeq(u16, [u8; 12]);

impl VmessNonceSeq {
    pub fn new(nonce: &[u8]) -> Self {
        let mut this = Self(0, [0; 12]);
        this.1[2..12].copy_from_slice(nonce);
        this
    }
}

impl NonceSeq for VmessNonceSeq {
    fn advance(&mut self) -> Option<[u8; 12]> {
        let o = self.0.to_be_bytes();
        self.0 = self.0.wrapping_add(1);

        self.1[0] = o[0];
        self.1[1] = o[1];

        Some(self.1)
    }
}

pub fn generate_chacha20poly1305_key(key: &[u8]) -> Vec<u8> {
    let key_1 = hash_bytes(HashKind::Md5, key);
    let key_2 = hash_bytes(HashKind::Md5, &key_1);
    [key_1, key_2].concat()
}
