use crate::crypto::*;
use crate::prelude::*;
use aead::AeadCipherKind;

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum SsAeadCipherKind {
    #[serde(rename = "aes-128-gcm")]
    Aes128Gcm,
    #[serde(rename = "aes-256-gcm")]
    Aes256Gcm,
}

impl Into<AeadCipherKind> for SsAeadCipherKind {
    fn into(self) -> AeadCipherKind {
        match self {
            SsAeadCipherKind::Aes128Gcm => AeadCipherKind::Aes128Gcm,
            SsAeadCipherKind::Aes256Gcm => AeadCipherKind::Aes256Gcm,
        }
    }
}

impl SsAeadCipherKind {
    fn derive_key(&self, password: &str) -> Bytes {
        let cipher_kind: AeadCipherKind = (*self).into();
        hashing::evp_bytes_to_key(
            hashing::HashKind::Md5,
            password.as_ref(),
            cipher_kind.key_len(),
        )
    }

    fn generate_salt(&self) -> Result<Bytes> {
        let cipher_kind: AeadCipherKind = (*self).into();
        let salt_len = cipher_kind.iv_len();
        let mut salt = BytesMut::with_capacity(salt_len);
        unsafe {
            salt.set_len(salt_len);
        }

        random::rand_bytes(&mut salt)?;
        Ok(salt.freeze())
    }
}
