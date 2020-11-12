use crate::crypto::*;

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum SsAeadCipherKind {
  #[serde(rename = "aes-128-gcm")]
  Aes128Gcm,
  #[serde(rename = "aes-192-gcm")]
  Aes192Gcm,
  #[serde(rename = "aes-256-gcm")]
  Aes256Gcm,
}