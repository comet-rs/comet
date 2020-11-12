use super::CrypterMode;
use crate::prelude::*;

pub trait StreamCrypter: Send + Sync {
  fn update(&mut self, in_out: &mut [u8]) -> Result<usize>;
}

pub enum StreamCipherKind {
  Aes128Cfb,
  Aes192Cfb,
  Aes256Cfb,
}

impl StreamCipherKind {
  fn cipher_info(&self) -> (usize, Option<usize>, usize) {
    match self {
      StreamCipherKind::Aes128Cfb => (16, Some(16), 16),
      StreamCipherKind::Aes192Cfb => (24, Some(16), 16),
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
      Ok(match self {
        StreamCipherKind::Aes128Cfb | StreamCipherKind::Aes192Cfb | StreamCipherKind::Aes256Cfb => {
          Box::new(rust::RustCfbCrypter::new(mode, self, key, iv)?)
        }
      })
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
        StreamCipherKind::Aes128Cfb => symm::Cipher::aes_128_cfb128(),
        StreamCipherKind::Aes192Cfb => symm::Cipher::aes_192_cfb128(),
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
      let inner: Box<dyn StreamCipher + Send + Sync> = match kind {
        StreamCipherKind::Aes128Cfb => Box::new(Cfb::<aes::Aes128>::new_var(key, iv).unwrap()),
        StreamCipherKind::Aes192Cfb => Box::new(Cfb::<aes::Aes192>::new_var(key, iv).unwrap()),
        StreamCipherKind::Aes256Cfb => Box::new(Cfb::<aes::Aes256>::new_var(key, iv).unwrap()),
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
