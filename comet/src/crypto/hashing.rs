use crate::prelude::*;
use crypto2::hash as ss_hash;
use crypto2::mac as ss_mac;
use std::cmp::min;

#[derive(Debug, Copy, Clone)]
pub enum HashKind {
    Md5,
    Sha1,
}

impl HashKind {
    pub fn output_len(&self) -> usize {
        match self {
            HashKind::Md5 => 16,
            HashKind::Sha1 => 20,
        }
    }
}

pub trait Hasher {
    fn update(&mut self, data: &[u8]);
    fn finish(&mut self) -> Bytes;
}

#[derive(Clone)]
enum SsHasherInner {
    Md5(ss_hash::Md5),
    Sha1(ss_hash::Sha1),
}

impl SsHasherInner {
    fn new(kind: HashKind) -> Self {
        match kind {
            HashKind::Md5 => Self::Md5(ss_hash::Md5::new()),
            HashKind::Sha1 => Self::Sha1(ss_hash::Sha1::new()),
        }
    }
    fn update(&mut self, data: &[u8]) {
        match self {
            Self::Md5(s) => s.update(data),
            Self::Sha1(s) => s.update(data),
        }
    }

    fn finish(self) -> Bytes {
        match self {
            Self::Md5(s) => Bytes::copy_from_slice(&s.finalize()),
            Self::Sha1(s) => Bytes::copy_from_slice(&s.finalize()),
        }
    }
}

#[derive(Clone)]
pub struct SsHasher(Option<SsHasherInner>);

impl SsHasher {
    pub fn new(kind: HashKind) -> Self {
        Self(Some(SsHasherInner::new(kind)))
    }
}

impl Hasher for SsHasher {
    fn update(&mut self, data: &[u8]) {
        let inner = self.0.as_mut().unwrap();
        inner.update(data);
    }

    fn finish(&mut self) -> Bytes {
        let inner = self.0.take().unwrap();
        inner.finish()
    }
}

pub trait Signer {
    fn update(&mut self, data: &[u8]);
    fn finish(&mut self) -> Bytes;
}

#[derive(Clone)]
pub enum SsSignerInner {
    Md5(ss_mac::HmacMd5),
    Sha1(ss_mac::HmacSha1),
}

impl SsSignerInner {
    fn new(kind: HashKind, key: &[u8]) -> Self {
        match kind {
            HashKind::Md5 => Self::Md5(ss_mac::HmacMd5::new(key)),
            HashKind::Sha1 => Self::Sha1(ss_mac::HmacSha1::new(key)),
        }
    }

    fn update(&mut self, data: &[u8]) {
        match self {
            Self::Md5(s) => s.update(data),
            Self::Sha1(s) => s.update(data),
        }
    }

    fn finish(self) -> Bytes {
        match self {
            Self::Md5(s) => Bytes::copy_from_slice(&s.finalize()),
            Self::Sha1(s) => Bytes::copy_from_slice(&s.finalize()),
        }
    }
}

pub struct SsSigner(Option<SsSignerInner>);

impl SsSigner {
    pub fn new(kind: HashKind, key: &[u8]) -> Self {
        Self(Some(SsSignerInner::new(kind, key)))
    }
}

impl Signer for SsSigner {
    fn update(&mut self, data: &[u8]) {
        let inner = self.0.as_mut().unwrap();
        inner.update(data);
    }

    fn finish(&mut self) -> Bytes {
        let inner = self.0.take().unwrap();
        inner.finish()
    }
}

pub fn new_hasher(kind: HashKind) -> SsHasher {
    SsHasher::new(kind)
}

pub fn hash_bytes(kind: HashKind, input: &[u8]) -> Bytes {
    let mut hasher = new_hasher(kind);
    hasher.update(input);
    hasher.finish()
}

pub fn new_signer(kind: HashKind, key: &[u8]) -> SsSigner {
    SsSigner::new(kind, key)
}

pub fn sign_bytes(kind: HashKind, key: &[u8], input: &[u8]) -> Bytes {
    let mut signer = new_signer(kind, key);
    signer.update(input);
    signer.finish()
}

pub fn evp_bytes_to_key(kind: HashKind, input: &[u8], len: usize) -> Bytes {
    let mut buf = BytesMut::with_capacity(len);
    let mut last_hash: Option<Bytes> = None;

    while buf.len() < len {
        let mut hasher = new_hasher(kind);
        if let Some(last_hash) = last_hash {
            hasher.update(&last_hash);
        }
        hasher.update(input);
        let hash = hasher.finish();
        let write_len = min(hash.len(), len - buf.len());
        buf.put_slice(&hash[0..write_len]);
        last_hash = Some(hash);
    }

    buf.freeze()
}

#[cfg(test)]
mod test {
    use super::{evp_bytes_to_key, HashKind};

    #[test]
    fn bytes_to_key() {
        let key = evp_bytes_to_key(HashKind::Md5, b"abc", 32);
        assert_eq!(
            &b"\x90\x01P\x98<\xd2O\xb0\xd6\x96?}(\xe1\x7fr\xea\x0b1\xe1\x08z\"\xbcS\x94\xa6cnn\xd3K"[..],
            &key[..]
        );
    }
}
