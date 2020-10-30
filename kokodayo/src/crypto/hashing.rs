use crate::prelude::*;
use openssl::hash::{Hasher as OpensslHasher, MessageDigest};
use std::cmp::min;

#[derive(Debug, Copy, Clone)]
pub enum HashKind {
  Md5,
  Sha1,
}

pub trait Hasher {
  fn update(&mut self, data: &[u8]) -> Result<()>;
  fn finish(&mut self) -> Result<Vec<u8>>;
}

impl Hasher for OpensslHasher {
  fn update(&mut self, data: &[u8]) -> Result<()> {
    Ok(OpensslHasher::update(self, data)?)
  }

  fn finish(&mut self) -> Result<Vec<u8>> {
    Ok(OpensslHasher::finish(self)?.to_vec())
  }
}

pub fn new_hasher(kind: HashKind) -> Result<Box<dyn Hasher>> {
  let hasher = Box::new(OpensslHasher::new(match kind {
    HashKind::Md5 => MessageDigest::md5(),
    HashKind::Sha1 => MessageDigest::sha1(),
  })?);
  Ok(hasher)
}

pub fn hash_bytes(kind: HashKind, input: &[u8]) -> Result<Vec<u8>> {
  let mut hasher = new_hasher(kind)?;
  hasher.update(input)?;
  Ok(hasher.finish()?)
}

pub fn evp_bytes_to_key(kind: HashKind, input: &[u8], len: usize) -> Result<Bytes> {
  let mut buf = BytesMut::with_capacity(len);
  let mut last_hash: Option<Vec<u8>> = None;

  while buf.len() < len {
    let mut hasher = new_hasher(kind)?;
    if let Some(last_hash) = last_hash {
      hasher.update(&last_hash)?;
    }
    hasher.update(input)?;
    let hash = hasher.finish()?;
    let write_len = min(hash.len(), len - buf.len());
    buf.put_slice(&hash[0..write_len]);
    last_hash = Some(hash);
  }

  Ok(buf.freeze())
}

#[cfg(test)]
mod test {
  use super::{evp_bytes_to_key, HashKind};

  #[test]
  fn bytes_to_key() {
    let key = evp_bytes_to_key(HashKind::Md5, b"abc", 32).unwrap();
    assert_eq!(
      &b"\x90\x01P\x98<\xd2O\xb0\xd6\x96?}(\xe1\x7fr\xea\x0b1\xe1\x08z\"\xbcS\x94\xa6cnn\xd3K"[..],
      &key[..]
    );
  }
}
