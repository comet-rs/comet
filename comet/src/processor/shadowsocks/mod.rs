mod aead_cipher;
mod auth;
mod handshake;
mod obfs;
mod stream_cipher;

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

#[derive(Debug, Clone, Deserialize, Copy)]
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
    Config {
        server: Option<DestAddr>,
        #[serde(flatten)]
        inner: ClientConfig,
    },
    Url {
        url: String,
    },
}

pub struct SsrClientProcessor {
    obfs: Option<Arc<dyn Processor>>,
    cipher: Option<Arc<dyn Processor>>,
    auth: Option<Arc<dyn Processor>>,
    handshake: Arc<ShadowsocksClientHandshakeProcessor>,
    dest: Option<DestAddr>,
}

impl SsrClientProcessor {
    fn new(config: ClientConfig, dest: Option<DestAddr>) -> Result<Self> {
        let handshake = handshake::ShadowsocksClientHandshakeProcessor::new();

        let protocol_param = config.protocol_param.as_str();

        let auth: Option<Arc<dyn Processor>> = match config.protocol {
            ProtocolType::Origin => None,
            ProtocolType::AuthAes128Md5 => Some(Arc::new(auth::SsrClientAuthProcessor::new_param(
                auth::SsrClientAuthType::AuthAes128Md5,
                protocol_param,
            )?)),
            ProtocolType::AuthAes128Sha1 => {
                Some(Arc::new(auth::SsrClientAuthProcessor::new_param(
                    auth::SsrClientAuthType::AuthAes128Sha1,
                    protocol_param,
                )?))
            }
        };

        let cipher: Option<Arc<dyn Processor>> = match config.method {
            MethodType::Aes256Cfb => Some(Arc::new(stream_cipher::ClientProcessor::new(
                stream_cipher::SsStreamCipherKind::Aes256Cfb,
                config.password.as_str(),
            ))),
        };

        let obfs: Option<Arc<dyn Processor>> = match config.obfs {
            ObfsType::Plain => None,
            _ => Some(Arc::new(obfs::ClientProcessor::new_param(
                config.obfs,
                config.obfs_param.as_str(),
                dest.as_ref().and_then(|d| d.port),
            ))),
        };

        Ok(Self {
            obfs,
            cipher,
            auth,
            handshake: Arc::new(handshake),
            dest,
        })
    }
}

#[async_trait]
impl Processor for SsrClientProcessor {
    async fn prepare(self: Arc<Self>, conn: &mut Connection, _ctx: AppContextRef) -> Result<()> {
        if let Some(dest) = &self.dest {
            conn.dest_addr = dest.clone();
        }
        Ok(())
    }
    async fn process(
        self: Arc<Self>,
        mut stream: ProxyStream,
        conn: &mut Connection,
        ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        if let Some(obfs) = &self.obfs {
            stream = obfs.clone().process(stream, conn, ctx.clone()).await?;
        }
        if let Some(cipher) = &self.cipher {
            stream = cipher.clone().process(stream, conn, ctx.clone()).await?;
        }
        if let Some(auth) = &self.auth {
            stream = auth.clone().process(stream, conn, ctx.clone()).await?;
        }
        stream = self.handshake.clone().process(stream, conn, ctx).await?;
        if let Some(dest) = &self.dest {
            conn.dest_addr = dest.clone();
        }
        Ok(stream)
    }
}

pub fn register(plumber: &mut Plumber) {
    auth::register(plumber);
    handshake::register(plumber);
    obfs::register(plumber);
    stream_cipher::register(plumber);

    plumber.register("ssr_client", |config, _| {
        let config: SsrClientConfig = from_value(config)?;
        let (config, dest) = match config {
            SsrClientConfig::Config { inner, server } => (inner, server),
            SsrClientConfig::Url { url } => {
                let item = parse_url(&url)?;
                (item.config, Some(item.dest))
            }
        };

        Ok(Box::new(SsrClientProcessor::new(config, dest)?))
    });
}

#[derive(Debug, Clone)]
pub struct SsrUrlItem {
    pub dest: DestAddr,
    pub config: ClientConfig,
    pub extras: HashMap<SmolStr, String>,
}

pub fn parse_url(url: &str) -> Result<SsrUrlItem> {
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

    Ok(SsrUrlItem {
        dest,
        config,
        extras,
    })
}

pub fn parse_subscription(content: &str) -> Result<Vec<SsrUrlItem>> {
    let decoded = base64::decode(content)?;
    let decoded_str = std::str::from_utf8(&decoded)?;
    decoded_str.lines().map(parse_url).collect()
}
