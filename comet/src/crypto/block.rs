use super::CrypterMode;
use crate::prelude::*;

pub trait BlockCrypter: Send + Sync {
  fn update(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize>;
}

pub enum BlockCipherKind {
  Aes128Cbc,
}

impl BlockCipherKind {
  fn cipher_info(&self) -> (usize, Option<usize>, usize) {
    match self {
      BlockCipherKind::Aes128Cbc => (16, Some(16), 16),
    }
  }

  pub fn key_len(&self) -> usize {
    self.cipher_info().0
  }

  pub fn iv_len(&self) -> Option<usize> {
    self.cipher_info().1
  }

  pub fn block_size(&self) -> usize {
    self.cipher_info().2
  }

  pub fn to_crypter<'a>(
    &self,
    mode: CrypterMode,
    key: &'a [u8],
    iv: &'a [u8],
    padding: bool,
  ) -> Result<Box<dyn BlockCrypter>> {
    #[cfg(target_os = "windows")]
    let crypter = rust::RustBlockCrypter::new(mode, self, key, iv, padding)?;
    #[cfg(not(target_os = "windows"))]
    let crypter = openssl::new_crypter(mode, self, key, iv, padding)?;
    Ok(Box::new(crypter))
  }
}

#[cfg(not(target_os = "windows"))]
mod openssl {
  use super::{BlockCipherKind, BlockCrypter, CrypterMode};
  use crate::prelude::*;
  use ::openssl::symm;

  pub fn new_crypter<'a>(
    mode: CrypterMode,
    kind: &BlockCipherKind,
    key: &'a [u8],
    iv: &'a [u8],
    padding: bool,
  ) -> Result<symm::Crypter> {
    let openssl_mode = match mode {
      CrypterMode::Decrypt => symm::Mode::Decrypt,
      CrypterMode::Encrypt => symm::Mode::Encrypt,
    };
    let mut crypter = symm::Crypter::new(
      match kind {
        BlockCipherKind::Aes128Cbc => symm::Cipher::aes_128_cbc(),
      },
      openssl_mode,
      key,
      Some(iv),
    )?;
    crypter.pad(padding);
    Ok(crypter)
  }

  impl BlockCrypter for symm::Crypter {
    fn update(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize> {
      Ok(self.update(input, output)?)
    }
  }
}

mod rust {
  use super::{BlockCipherKind, BlockCrypter, CrypterMode};
  use crate::prelude::*;
  use aes::Aes128;
  use block_modes::block_padding::{NoPadding, Pkcs7};
  use block_modes::{BlockMode, Cbc};

  enum BlockCrypterInner {
    Aes128CbcNoPadding(Cbc<Aes128, NoPadding>),
  }

  impl BlockCrypterInner {
    fn encrypt(self, buffer: &[u8]) -> Result<Vec<u8>> {
      Ok(match self {
        BlockCrypterInner::Aes128CbcNoPadding(e) => e.encrypt_vec(buffer),
      })
    }

    fn decrypt(self, buffer: &[u8]) -> Result<Vec<u8>> {
      Ok(match self {
        BlockCrypterInner::Aes128CbcNoPadding(e) => e.decrypt_vec(buffer)?,
      })
    }
  }

  pub struct RustBlockCrypter {
    mode: CrypterMode,
    inner: Option<BlockCrypterInner>,
  }

  impl RustBlockCrypter {
    pub fn new<'a>(
      mode: CrypterMode,
      kind: &BlockCipherKind,
      key: &'a [u8],
      iv: &'a [u8],
      padding: bool,
    ) -> Result<Self> {
      assert!(!padding);
      let crypter = match kind {
        BlockCipherKind::Aes128Cbc => {
          BlockCrypterInner::Aes128CbcNoPadding(Cbc::<Aes128, NoPadding>::new_var(key, iv)?)
        }
      };

      Ok(Self {
        mode,
        inner: Some(crypter),
      })
    }
  }

  impl BlockCrypter for RustBlockCrypter {
    fn update(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize> {
      let inner = self.inner.take().unwrap();

      let ret = match self.mode {
        CrypterMode::Decrypt => inner.decrypt(input)?,
        CrypterMode::Encrypt => inner.encrypt(input)?,
      };
      assert!(output.len() >= ret.len());

      output[..ret.len()].copy_from_slice(&ret);

      Ok(ret.len())
    }
  }
}
