use super::CrypterMode;
use crate::prelude::*;
use openssl::symm;
use std::slice;

pub trait StreamCrypter {
  fn update(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize>;
  fn update_in_place(&mut self, in_out: &mut [u8]) -> Result<usize> {
    let in_raw = in_out.as_ptr();
    self.update(
      unsafe { slice::from_raw_parts(in_raw, in_out.len()) },
      in_out,
    )
  }
}

impl StreamCrypter for symm::Crypter {
  fn update(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize> {
    Ok(self.update(input, output)?)
  }
}

pub enum StreamCipherKind {
  Aes256Cfb,
}

impl StreamCipherKind {
  fn get_openssl_cipher(&self) -> symm::Cipher {
    match self {
      StreamCipherKind::Aes256Cfb => symm::Cipher::aes_256_cfb128(),
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
  ) -> Result<Box<dyn StreamCrypter + Send>> {
    let openssl_mode = match mode {
      CrypterMode::Decrypt => symm::Mode::Decrypt,
      CrypterMode::Encrypt => symm::Mode::Encrypt,
    };

    let crypter = symm::Crypter::new(self.get_openssl_cipher(), openssl_mode, key, iv.into())?;
    Ok(Box::new(crypter))
  }
}
