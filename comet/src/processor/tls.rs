use tokio_rustls::{
    rustls,
    webpki::{DNSName, DNSNameRef},
    TlsConnector,
};

use crate::prelude::*;

pub fn register(plumber: &mut Plumber) {
    plumber.register("tls_client", |conf, _| {
        let config: ClientConfig = from_value(conf)?;
        Ok(Box::new(ClientProcessor::new(config)?))
    });
}

#[derive(Debug, Deserialize, Clone)]
pub struct ClientConfig {
    sni: SmolStr,
}
pub struct ClientProcessor {
    connector: TlsConnector,
    sni: DNSName,
    config: ClientConfig,
}

impl ClientProcessor {
    fn new(config: ClientConfig) -> Result<Self> {
        let mut rustls_config = rustls::ClientConfig::new();
        rustls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
        rustls_config.ct_logs = Some(&ct_logs::LOGS);
        rustls_config
            .root_store
            .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);

        let sni = DNSNameRef::try_from_ascii_str(&config.sni)?;

        Ok(Self {
            connector: Arc::new(rustls_config).into(),
            sni: sni.to_owned(),
            config,
        })
    }
}

#[async_trait]
impl Processor for ClientProcessor {
    async fn prepare(self: Arc<Self>, _conn: &mut Connection, _ctx: AppContextRef) -> Result<()> {
        Ok(())
    }

    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        _conn: &mut Connection,
        _ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        let stream = stream.into_tcp()?;
        let tls_stream = self.connector.connect(self.sni.as_ref(), stream).await?;
        Ok(RWPair::new(tls_stream).into())
    }
}
