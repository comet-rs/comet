use super::CrypterMode;
use crate::prelude::*;

pub trait StreamCrypter: Send + Sync {
  fn update(&mut self, in_out: &mut [u8]) -> Result<usize>;
}

pub enum StreamCipherKind {
  Aes256Cfb,
}

impl StreamCipherKind {
  fn cipher_info(&self) -> (usize, Option<usize>, usize) {
    match self {
      StreamCipherKind::Aes256Cfb => (32, Some(16), 16),
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
  ) -> Result<Box<dyn StreamCrypter>> {
    #[cfg(target_os = "windows")]
    {
      let crypter = rust::RustCfbCrypter::new(mode, self, key, iv)?;
      Ok(Box::new(crypter))
    }
    #[cfg(not(target_os = "windows"))]
    {
      let crypter = openssl::new_crypter(mode, self, key, iv)?;
      Ok(Box::new(crypter))
    }
  }
}

#[cfg(not(target_os = "windows"))]
mod openssl {
  use super::{CrypterMode, StreamCipherKind, StreamCrypter};
  use crate::prelude::*;
  use ::openssl::symm;
  use std::slice;
  pub fn new_crypter<'a>(
    mode: CrypterMode,
    kind: &StreamCipherKind,
    key: &'a [u8],
    iv: &'a [u8],
  ) -> Result<symm::Crypter> {
    let openssl_mode = match mode {
      CrypterMode::Decrypt => symm::Mode::Decrypt,
      CrypterMode::Encrypt => symm::Mode::Encrypt,
    };
    let crypter = symm::Crypter::new(
      match kind {
        StreamCipherKind::Aes256Cfb => symm::Cipher::aes_256_cfb128(),
      },
      openssl_mode,
      key,
      Some(iv),
    )?;
    Ok(crypter)
  }

  impl StreamCrypter for symm::Crypter {
    fn update(&mut self, in_out: &mut [u8]) -> Result<usize> {
      let in_raw = in_out.as_ptr();
      Ok(self.update(
        unsafe { slice::from_raw_parts(in_raw, in_out.len()) },
        in_out,
      )?)
    }
  }
}

#[cfg(target_os = "windows")]
mod rust {
  use super::{CrypterMode, StreamCipherKind, StreamCrypter};
  use crate::prelude::*;
  use aes::{Aes128, Aes192, Aes256};
  use cfb_mode::Cfb;
  use cipher::{NewStreamCipher, StreamCipher};

  pub struct RustCfbCrypter {
    mode: CrypterMode,
    inner: Box<dyn StreamCipher + Send + Sync>,
  }

  impl RustCfbCrypter {
    pub fn new<'a>(
      mode: CrypterMode,
      kind: &StreamCipherKind,
      key: &'a [u8],
      iv: &'a [u8],
    ) -> Result<Self> {
      let inner = match kind {
        StreamCipherKind::Aes256Cfb => Box::new(Cfb::<Aes256>::new_var(key, iv).unwrap()),
      };

      Ok(Self { mode, inner })
    }
  }

  impl StreamCrypter for RustCfbCrypter {
    fn update(&mut self, in_out: &mut [u8]) -> Result<usize> {
      match self.mode {
        CrypterMode::Decrypt => self.inner.decrypt(in_out),
        CrypterMode::Encrypt => self.inner.encrypt(in_out),
      }
      Ok(in_out.len())
    }
  }
}
