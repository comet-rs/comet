#![allow(dead_code, unused_variables)]
pub mod auth;
pub mod handshake;
pub mod obfs;
pub mod stream_cipher;

use crate::prelude::*;
use crate::Plumber;
use handshake::ShadowsocksClientHandshakeProcessor;

#[derive(Debug, Clone, Deserialize)]
enum MethodType {
    #[serde(rename = "aes-256-cfb")]
    Aes256Cfb,
}

#[derive(Debug, Clone, Deserialize)]
enum ProtocolType {
    #[serde(rename = "origin")]
    Origin,
    #[serde(rename = "auth_aes128_md5")]
    AuthAes128Md5,
    #[serde(rename = "auth_aes128_sha1")]
    AuthAes128Sha1,
}

impl Default for ProtocolType {
    fn default() -> Self {
        Self::Origin
    }
}

#[derive(Debug, Clone, Deserialize)]
enum ObfsType {
    #[serde(rename = "plain")]
    Plain,
    #[serde(rename = "http_simple")]
    HttpSimple,
}

impl Default for ObfsType {
    fn default() -> Self {
        Self::Plain
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SsrClientConfig {
    password: SmolStr,
    method: MethodType,
    #[serde(default)]
    protocol: ProtocolType,
    #[serde(default)]
    protocol_param: SmolStr,
    #[serde(default)]
    obfs: ObfsType,
    #[serde(default)]
    obfs_param: SmolStr,
}

pub struct SsrClientProcessor {
    obfs: Option<Box<dyn Processor>>,
    cipher: Option<Box<dyn Processor>>,
    protocol: Option<Box<dyn Processor>>,
    handshake: ShadowsocksClientHandshakeProcessor,
}

impl SsrClientProcessor {
    fn new(config: YamlValue) -> Result<()> {
        let config: SsrClientConfig = from_value(config)?;
        let handshake = handshake::ShadowsocksClientHandshakeProcessor::new();

        let protocol_param = config.protocol_param.as_str();
        let protocol = match config.protocol {
            ProtocolType::Origin => None,
            ProtocolType::AuthAes128Md5 => Some(Box::new(auth::SsrClientAuthProcessor::new_param(
                auth::SsrClientAuthType::AuthAes128Md5,
                protocol_param,
            ))),
            ProtocolType::AuthAes128Sha1 => {
                Some(Box::new(auth::SsrClientAuthProcessor::new_param(
                    auth::SsrClientAuthType::AuthAes128Sha1,
                    protocol_param,
                )))
            }
        };
        Ok(())
    }
}

pub fn register(plumber: &mut Plumber) {
    auth::register(plumber);
    handshake::register(plumber);
    obfs::register(plumber);
    stream_cipher::register(plumber);
}
