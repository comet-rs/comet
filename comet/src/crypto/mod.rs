pub mod aead;
pub mod hashing;
pub mod rand;
pub mod stream;
pub mod block;

pub enum CrypterMode {
  Encrypt,
  Decrypt,
}

pub enum CipherKind {
  Stream(stream::StreamCipherKind),
}

impl CipherKind {
  pub fn key_len(&self) -> usize {
    match self {
      Self::Stream(b) => b.key_len(),
    }
  }

  pub fn iv_len(&self) -> Option<usize> {
    match self {
      Self::Stream(b) => b.iv_len(),
    }
  }

  pub fn block_size(&self) -> usize {
    match self {
      Self::Stream(b) => b.block_size(),
    }
  }
}
