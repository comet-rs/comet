use std::convert::TryFrom;

use tokio_rustls::{
    rustls::{self, ServerName},
    webpki::{DnsName, DnsNameRef},
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
    sni: DnsName,
    server_name: ServerName,
    config: ClientConfig,
}

impl ClientProcessor {
    fn new(config: ClientConfig) -> Result<Self> {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
            rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                ta.subject,
                ta.spki,
                ta.name_constraints,
            )
        }));
        let mut rustls_config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        rustls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        let sni = DnsNameRef::try_from_ascii_str(&config.sni)?;
        let server_name = ServerName::try_from(config.sni.as_str())?;

        Ok(Self {
            connector: Arc::new(rustls_config).into(),
            sni: sni.to_owned(),
            server_name,
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
        let tls_stream = self.connector.connect(self.server_name.clone(), stream).await?;
        Ok(RWPair::new(tls_stream).into())
    }
}
