use std::convert::TryInto;

use super::CrypterMode;
use crate::prelude::*;
use crypto2::blockmode;

pub trait BlockCrypter: Send + Sync {
    fn update(&mut self, in_out: &mut [u8]) -> Result<usize>;
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
        Ok(Box::new(SsCrypter::new(mode, self, key, iv)))
    }
}

trait BlockMode {
    fn iv_len(&self) -> usize;
    fn encrypt(&mut self, iv: &[u8], blocks: &mut [u8]);
    fn decrypt(&mut self, iv: &[u8], blocks: &mut [u8]);
}

impl BlockMode for blockmode::Aes128Cbc {
    fn iv_len(&self) -> usize {
        Self::IV_LEN
    }

    fn encrypt(&mut self, iv: &[u8], blocks: &mut [u8]) {
        let iv: [u8; Self::IV_LEN] = iv.try_into().expect("incorrect IV length");
        self.encrypt(&iv, blocks);
    }

    fn decrypt(&mut self, iv: &[u8], blocks: &mut [u8]) {
        let iv: [u8; Self::IV_LEN] = iv.try_into().expect("incorrect IV length");
        self.encrypt(&iv, blocks);
    }
}

struct SsCrypter {
    inner: Box<dyn BlockMode + Send + Sync>,
    mode: CrypterMode,
    iv: Vec<u8>,
}

impl SsCrypter {
    fn new(mode: CrypterMode, kind: &BlockCipherKind, key: &[u8], iv: &[u8]) -> Self {
        let inner = match kind {
            BlockCipherKind::Aes128Cbc => Box::new(blockmode::Aes128Cbc::new(key)),
        };

        return Self {
            inner,
            mode,
            iv: iv.into(),
        };
    }
}

impl BlockCrypter for SsCrypter {
    fn update(&mut self, in_out: &mut [u8]) -> Result<usize> {
        match self.mode {
            CrypterMode::Decrypt => {
                // CBC: Last ciphertext is IV
                let in_len = in_out.len();
                let iv_tmp = in_out[in_len - 16..in_len].to_vec();
                self.inner.decrypt(&self.iv, in_out);
                self.iv = iv_tmp;
            }
            CrypterMode::Encrypt => {
                // CBC: Last ciphertext is IV
                self.inner.encrypt(&self.iv, in_out);
                let out_len = in_out.len();
                self.iv.copy_from_slice(&in_out[out_len - 16..out_len]);
            }
        }

        Ok(0)
    }
}
