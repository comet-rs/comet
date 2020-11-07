pub mod auth;
pub mod handshake;
pub mod obfs;
pub mod stream_cipher;

use crate::prelude::*;
use crate::Plumber;
use handshake::ShadowsocksClientHandshakeProcessor;

enum ClientCipherProcessor {
  Stream(stream_cipher::ClientProcessor),
}

#[derive(Debug, Clone, Deserialize)]
enum MethodConfig {
  #[serde(rename = "aes-256-cfb")]
  Aes256Cfb,
}

enum ClientProtocolProcessor {
  Origin,
  AuthAes128(auth::SsrClientAuthProcessor),
}

#[derive(Debug, Clone, Deserialize)]
enum ProtocolConfig {
  #[serde(rename = "origin")]
  Origin,
  #[serde(rename = "auth_aes128_md5")]
  AuthAes128Md5,
  #[serde(rename = "auth_aes128_sha1")]
  AuthAes128Sha1,
}

impl Default for ProtocolConfig {
  fn default() -> Self {
    Self::Origin
  }
}

enum ClientObfsProcessor {
  Plain,
  HttpSimple(obfs::ClientProcessor),
}

#[derive(Debug, Clone, Deserialize)]
enum ObfsConfig {
  #[serde(rename = "plain")]
  Plain,
  #[serde(rename = "http_simple")]
  HttpSimple,
}

impl Default for ObfsConfig {
  fn default() -> Self {
    Self::Plain
  }
}

#[derive(Debug, Clone, Deserialize)]
struct SsrClientConfig {
  password: SmolStr,
  method: MethodConfig,
  #[serde(default)]
  protocol: ProtocolConfig,
  #[serde(default)]
  protocol_param: SmolStr,
  #[serde(default)]
  obfs: ObfsConfig,
  #[serde(default)]
  obfs_param: SmolStr,
}

pub struct SsrClientProcessor {
  obfs: ClientObfsProcessor,
  cipher: ClientCipherProcessor,
  protocol: ClientProtocolProcessor,
  handshake: ShadowsocksClientHandshakeProcessor,
}

impl SsrClientProcessor {
  fn new(config: YamlValue) -> Result<()> {
    let config: SsrClientConfig = from_value(config)?;
    Ok(())
  }
}

pub fn register(plumber: &mut Plumber) {
  auth::register(plumber);
  handshake::register(plumber);
  obfs::register(plumber);
  stream_cipher::register(plumber);
}
