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
        let crypter = ss_crypto::SsCrypter::new(mode, self, key, iv);
        Ok(Box::new(crypter))
    }
}

mod ss_crypto {
    use super::{CrypterMode, StreamCipherKind, StreamCrypter};
    use crate::prelude::*;
    use shadowsocks_crypto::v1::{Cipher, CipherKind as SsCipherKind};
    use std::sync::Mutex;

    pub struct SsCrypter {
        inner: Mutex<Cipher>,
        mode: CrypterMode,
    }

    impl SsCrypter {
        pub fn new(mode: CrypterMode, kind: &StreamCipherKind, key: &[u8], iv: &[u8]) -> Self {
            let ss_kind = match kind {
                StreamCipherKind::Aes128Cfb => SsCipherKind::AES_128_CFB128,
                StreamCipherKind::Aes192Cfb => SsCipherKind::AES_192_CFB128,
                StreamCipherKind::Aes256Cfb => SsCipherKind::AES_256_CFB128,
            };
            Self {
                inner: Mutex::new(Cipher::new(ss_kind, key, iv)),
                mode,
            }
        }
    }

    impl StreamCrypter for SsCrypter {
        fn update(&mut self, in_out: &mut [u8]) -> Result<usize> {
            let mut inner = self.inner.lock().unwrap();
            match self.mode {
                CrypterMode::Encrypt => inner.encrypt_packet(in_out),
                CrypterMode::Decrypt => {
                    let _ = inner.decrypt_packet(in_out);
                }
            }
            Ok(in_out.len())
        }
    }
}
