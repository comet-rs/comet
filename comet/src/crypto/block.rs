use super::CrypterMode;
use crate::prelude::*;
use openssl::symm;

pub trait BlockCrypter: Send + Sync {
  fn update(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize>;
}

impl BlockCrypter for symm::Crypter {
  fn update(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize> {
    Ok(self.update(input, output)?)
  }
}

pub enum BlockCipherKind {
  Aes128Cbc,
}

impl BlockCipherKind {
  fn get_openssl_cipher(&self) -> symm::Cipher {
    match self {
      BlockCipherKind::Aes128Cbc => symm::Cipher::aes_128_cbc(),
    }
  }

  pub fn key_len(&self) -> usize {
    self.get_openssl_cipher().key_len()
  }

  pub fn iv_len(&self) -> Option<usize> {
    self.get_openssl_cipher().iv_len()
  }

  pub fn block_size(&self) -> usize {
    self.get_openssl_cipher().block_size()
  }

  pub fn to_crypter<'a>(
    &self,
    mode: CrypterMode,
    key: &'a [u8],
    iv: impl Into<Option<&'a [u8]>>,
    padding: bool
  ) -> Result<Box<dyn BlockCrypter>> {
    let openssl_mode = match mode {
      CrypterMode::Decrypt => symm::Mode::Decrypt,
      CrypterMode::Encrypt => symm::Mode::Encrypt,
    };

    let mut crypter = symm::Crypter::new(self.get_openssl_cipher(), openssl_mode, key, iv.into())?;
    crypter.pad(padding);
    Ok(Box::new(crypter))
  }
}
