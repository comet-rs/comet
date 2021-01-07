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
        let crypter = windows::WinBlockCrypter::new(mode, self, key, iv, padding)?;
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

#[cfg(target_os = "windows")]
mod windows {
    use super::{BlockCipherKind, BlockCrypter, CrypterMode};
    use crate::prelude::*;
    use std::sync::Mutex;

    use win_crypto_ng::symmetric::{
        ChainingMode, SymmetricAlgorithm, SymmetricAlgorithmId, SymmetricAlgorithmKey,
    };

    pub struct WinBlockCrypter {
        mode: CrypterMode,
        inner: Mutex<SymmetricAlgorithmKey>,
        iv: Vec<u8>,
    }

    impl WinBlockCrypter {
        pub fn new<'a>(
            mode: CrypterMode,
            kind: &BlockCipherKind,
            key: &'a [u8],
            iv: &'a [u8],
            padding: bool,
        ) -> Result<Self> {
            assert!(!padding);

            let algo = match kind {
                BlockCipherKind::Aes128Cbc => {
                    SymmetricAlgorithm::open(SymmetricAlgorithmId::Aes, ChainingMode::Cbc)?
                }
            };

            let key = algo.new_key(key)?;

            Ok(Self {
                mode,
                inner: Mutex::new(key),
                iv: iv.to_vec(),
            })
        }
    }

    impl BlockCrypter for WinBlockCrypter {
        fn update(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize> {
            let inner = self.inner.lock().unwrap();

            let ret = match self.mode {
                CrypterMode::Decrypt => inner.decrypt(Some(&mut self.iv), input, None)?,
                CrypterMode::Encrypt => inner.encrypt(Some(&mut self.iv), input, None)?,
            };
            assert!(output.len() >= ret.len());

            output[..ret.len()].copy_from_slice(ret.as_slice());

            Ok(ret.len())
        }
    }
}
