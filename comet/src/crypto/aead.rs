use super::CrypterMode;
use crate::prelude::*;
use aeadcipher::AeadCipher;
use anyhow::anyhow;
use crypto2::aeadcipher;
use shadowsocks_crypto::v1::CipherKind as SsCipherKind;

pub trait AeadCrypter: Send + Sync {
    fn update(&mut self, in_out: &mut [u8]) -> Result<usize>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AeadCipherKind {
    Aes128Gcm,
    Aes256Gcm,
    Chacha20Poly1305,
}

impl AeadCipherKind {
    fn ss_cipher_kind(&self) -> SsCipherKind {
        match self {
            Self::Aes128Gcm => SsCipherKind::AES_128_GCM,
            Self::Aes256Gcm => SsCipherKind::AES_256_GCM,
            Self::Chacha20Poly1305 => SsCipherKind::CHACHA20_POLY1305,
        }
    }

    pub fn key_len(&self) -> usize {
        self.ss_cipher_kind().key_len()
    }

    pub fn iv_len(&self) -> usize {
        self.ss_cipher_kind().salt_len()
    }

    pub fn to_crypter<'a, N: NonceSeq + 'a>(
        &self,
        mode: CrypterMode,
        key: &'a [u8],
        nonce: N,
    ) -> Result<SsCrypter<N>> {
        let crypter = SsCrypter::new(mode, *self, key, nonce);
        Ok(crypter)
    }
}

enum SsCrypterInner {
    Aes128Gcm(aeadcipher::Aes128Gcm),
    Aes256Gcm(aeadcipher::Aes256Gcm),
    Chacha20Poly1305(aeadcipher::Chacha20Poly1305),
}

impl SsCrypterInner {
    fn new(key: &[u8], kind: AeadCipherKind) -> Self {
        match kind {
            AeadCipherKind::Aes128Gcm => Self::Aes128Gcm(aeadcipher::Aes128Gcm::aead_new(key)),
            AeadCipherKind::Aes256Gcm => Self::Aes256Gcm(aeadcipher::Aes256Gcm::aead_new(key)),
            AeadCipherKind::Chacha20Poly1305 => {
                Self::Chacha20Poly1305(aeadcipher::Chacha20Poly1305::aead_new(key))
            }
        }
    }

    fn tag_len(&self) -> usize {
        match self {
            SsCrypterInner::Aes128Gcm(_) => aeadcipher::Aes128Gcm::aead_tag_len(),
            SsCrypterInner::Aes256Gcm(_) => aeadcipher::Aes256Gcm::aead_tag_len(),
            SsCrypterInner::Chacha20Poly1305(_) => aeadcipher::Chacha20Poly1305::aead_tag_len(),
        }
    }

    fn encrypt_packet(&self, packet: &mut [u8], nonce: &[u8]) {
        match self {
            SsCrypterInner::Aes128Gcm(c) => c.aead_encrypt_slice(nonce, &[], packet),
            SsCrypterInner::Aes256Gcm(c) => c.aead_encrypt_slice(nonce, &[], packet),
            SsCrypterInner::Chacha20Poly1305(c) => c.aead_encrypt_slice(nonce, &[], packet),
        }
    }

    fn decrypt_packet(&self, packet: &mut [u8], nonce: &[u8]) -> bool {
        match self {
            SsCrypterInner::Aes128Gcm(c) => c.aead_decrypt_slice(nonce, &[], packet),
            SsCrypterInner::Aes256Gcm(c) => c.aead_decrypt_slice(nonce, &[], packet),
            SsCrypterInner::Chacha20Poly1305(c) => c.aead_decrypt_slice(nonce, &[], packet),
        }
    }
}

pub struct SsCrypter<N> {
    inner: SsCrypterInner,
    mode: CrypterMode,
    nonce: N,
}

impl<N> std::fmt::Debug for SsCrypter<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AEADSsCrypter")
    }
}

impl<N: NonceSeq> SsCrypter<N> {
    fn new(mode: CrypterMode, kind: AeadCipherKind, key: &[u8], nonce: N) -> Self {
        Self {
            inner: SsCrypterInner::new(key, kind),
            mode,
            nonce,
        }
    }

    pub fn tag_len(&self) -> usize {
        self.inner.tag_len()
    }
}

impl<N: NonceSeq> AeadCrypter for SsCrypter<N> {
    fn update(&mut self, in_out: &mut [u8]) -> Result<usize> {
        let tag_len = self.inner.tag_len();
        debug_assert!(in_out.len() >= tag_len);

        let nonce = self
            .nonce
            .advance()
            .ok_or_else(|| anyhow!("Nonce generation failed"))?;

        match self.mode {
            CrypterMode::Encrypt => {
                self.inner.encrypt_packet(in_out, &nonce[..]);
                Ok(in_out.len())
            }
            CrypterMode::Decrypt => {
                if self.inner.decrypt_packet(in_out, &nonce[..]) {
                    Ok(in_out.len() - tag_len)
                } else {
                    Err(anyhow!("Decryption failed"))
                }
            }
        }
    }
}

pub trait NonceSeq: Send + Sync {
    fn advance(&mut self) -> Option<[u8; 12]>;
}
