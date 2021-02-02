pub mod auth;
pub mod handshake;
pub mod obfs;
pub mod stream_cipher;

use crate::Plumber;
use crate::{prelude::*, utils::urlsafe_base64_decode_string};
use handshake::ShadowsocksClientHandshakeProcessor;

use anyhow::anyhow;

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
#[serde(rename_all(deserialize = "snake_case"))]
enum ObfsType {
    Plain,
    HttpSimple,
}

impl Default for ObfsType {
    fn default() -> Self {
        Self::Plain
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClientConfig {
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

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum SsrClientConfig {
    Config(ClientConfig),
    Url { url: String },
}

impl SsrClientConfig {
    fn into_config(self) -> Result<ClientConfig> {
        match self {
            Self::Config(c) => Ok(c),
            Self::Url { url } => Ok(parse_url(&url)?.1),
        }
    }
}

pub struct SsrClientProcessor {
    obfs: Option<Box<dyn Processor>>,
    cipher: Option<Box<dyn Processor>>,
    protocol: Option<Box<dyn Processor>>,
    handshake: ShadowsocksClientHandshakeProcessor,
}

impl SsrClientProcessor {
    fn new(config: ClientConfig) -> Result<Self> {
        let handshake = handshake::ShadowsocksClientHandshakeProcessor::new();

        let protocol_param = config.protocol_param.as_str();

        let protocol: Option<Box<dyn Processor>> = match config.protocol {
            ProtocolType::Origin => None,
            ProtocolType::AuthAes128Md5 => Some(Box::new(auth::SsrClientAuthProcessor::new_param(
                auth::SsrClientAuthType::AuthAes128Md5,
                protocol_param,
            )?)),
            ProtocolType::AuthAes128Sha1 => {
                Some(Box::new(auth::SsrClientAuthProcessor::new_param(
                    auth::SsrClientAuthType::AuthAes128Sha1,
                    protocol_param,
                )?))
            }
        };

        let cipher: Option<Box<dyn Processor>> = match config.method {
            MethodType::Aes256Cfb => Some(Box::new(stream_cipher::ClientProcessor::new(
                stream_cipher::SsStreamCipherKind::Aes256Cfb,
                config.password.as_str(),
            ))),
        };

        let obfs: Option<Box<dyn Processor>> = match config.obfs {
            ObfsType::Plain => None,
            ObfsType::HttpSimple => None,
        };

        Ok(Self {
            obfs,
            cipher,
            protocol,
            handshake,
        })
    }
}

#[async_trait]
impl Processor for SsrClientProcessor {
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        conn: &mut Connection,
        ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        todo!()
    }
}

pub fn register(plumber: &mut Plumber) {
    auth::register(plumber);
    handshake::register(plumber);
    obfs::register(plumber);
    stream_cipher::register(plumber);
    plumber.register("ssr_client", |config, _| {
        let config: SsrClientConfig = from_value(config)?;
        let config = match config {
            SsrClientConfig::Config(c) => c,
            SsrClientConfig::Url { url } => parse_url(&url)?.1,
        };
        Ok(Box::new(SsrClientProcessor::new(config)?))
    });
}

pub fn parse_url(url: &str) -> Result<(DestAddr, ClientConfig, HashMap<SmolStr, String>)> {
    let mut dest = DestAddr::default();

    let encoded = url
        .strip_prefix("ssr://")
        .ok_or_else(|| anyhow!("Not SSR URL"))?;
    let decoded = urlsafe_base64_decode_string(encoded)?;

    let mut split = decoded.splitn(2, "/?");
    let main_cfg = split.next().unwrap();

    let mut extras = if let Some(qs) = split.next() {
        let parsed_qs = url::form_urlencoded::parse(qs.as_bytes()).map(|(k, v)| {
            let v = urlsafe_base64_decode_string(&v.as_bytes())?;
            Ok((SmolStr::from(k), v))
        });
        parsed_qs.collect::<Result<HashMap<_, _>>>()?
    } else {
        HashMap::new()
    };

    let empty_str_fn = || "".to_string();

    let protocol_param = extras.remove("protoparam").unwrap_or_else(empty_str_fn);
    let obfs_param = extras.remove("obfsparam").unwrap_or_else(empty_str_fn);

    let invalid_url_fn = || anyhow!("Invalid SSR URL");

    let mut main_split = main_cfg.split(':');

    dest.set_host_from_str(main_split.next().ok_or_else(invalid_url_fn)?);
    dest.set_port(
        main_split
            .next()
            .ok_or_else(invalid_url_fn)
            .and_then(|s| Ok(u16::from_str_radix(s, 10)?))?,
    );

    let protocol: ProtocolType = from_value(YamlValue::String(
        main_split.next().ok_or_else(invalid_url_fn)?.into(),
    ))?;
    let method: MethodType = from_value(YamlValue::String(
        main_split.next().ok_or_else(invalid_url_fn)?.into(),
    ))?;
    let obfs: ObfsType = from_value(YamlValue::String(
        main_split.next().ok_or_else(invalid_url_fn)?.into(),
    ))?;

    let password = urlsafe_base64_decode_string(main_split.next().ok_or_else(invalid_url_fn)?)?;

    let config = ClientConfig {
        password: password.into(),
        method,
        protocol,
        protocol_param: protocol_param.into(),
        obfs,
        obfs_param: obfs_param.into(),
    };

    Ok((dest, config, extras))
}
