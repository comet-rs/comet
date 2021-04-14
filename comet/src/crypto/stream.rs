use super::CrypterMode;
use crate::prelude::*;
use shadowsocks_crypto::v1::{Cipher, CipherKind as SsCipherKind};

pub trait StreamCrypter: Send + Sync {
    fn update(&mut self, in_out: &mut [u8]) -> Result<usize>;
}

pub enum StreamCipherKind {
    Aes128Cfb,
    Aes192Cfb,
    Aes256Cfb,
}

impl StreamCipherKind {
    fn ss_cipher_kind(&self) -> SsCipherKind {
        match self {
            StreamCipherKind::Aes128Cfb => SsCipherKind::AES_128_CFB128,
            StreamCipherKind::Aes192Cfb => SsCipherKind::AES_192_CFB128,
            StreamCipherKind::Aes256Cfb => SsCipherKind::AES_256_CFB128,
        }
    }

    pub fn key_len(&self) -> usize {
        self.ss_cipher_kind().key_len()
    }

    pub fn iv_len(&self) -> usize {
        self.ss_cipher_kind().iv_len()
    }

    pub fn to_crypter<'a>(
        &self,
        mode: CrypterMode,
        key: &'a [u8],
        iv: &'a [u8],
    ) -> Result<SsCrypter> {
        let crypter = SsCrypter::new(mode, self, key, iv);
        Ok(crypter)
    }
}

pub struct SsCrypter {
    inner: Cipher,
    mode: CrypterMode,
}

impl std::fmt::Debug for SsCrypter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SsCryper ({:?})", self.mode)
    }
}

/// This is prefectly fine, they just forgot to have `+Sync`.
unsafe impl Sync for SsCrypter {}

impl SsCrypter {
    fn new(mode: CrypterMode, kind: &StreamCipherKind, key: &[u8], iv: &[u8]) -> Self {
        Self {
            inner: Cipher::new(kind.ss_cipher_kind(), key, iv),
            mode,
        }
    }
}

impl StreamCrypter for SsCrypter {
    fn update(&mut self, in_out: &mut [u8]) -> Result<usize> {
        match self.mode {
            CrypterMode::Encrypt => self.inner.encrypt_packet(in_out),
            CrypterMode::Decrypt => {
                // Never fails in stream cipher
                let _ = self.inner.decrypt_packet(in_out);
            }
        }
        Ok(in_out.len())
    }
}
