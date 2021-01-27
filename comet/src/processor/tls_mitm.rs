use std::{collections::HashSet, sync::RwLock};

use crate::prelude::*;
use rcgen::{BasicConstraints, Certificate, CertificateParams, DnType, IsCa};
use tokio_rustls::{
    rustls::{
        sign::{any_supported_type, CertifiedKey},
        Certificate as RustlsCert, NoClientAuth, PrivateKey, ResolvesServerCert,
        ResolvesServerCertUsingSNI, ServerConfig,
    },
    TlsAcceptor,
};

pub fn register(plumber: &mut Plumber) {
    plumber.register("tls_mitm", |_, pipe_tag| {
        Ok(Box::new(TlsMitmProcessor::new(pipe_tag)?))
    });
}

fn new_ca_cert() -> Result<Certificate> {
    let mut ca_param: CertificateParams = Default::default();
    ca_param.distinguished_name.remove(DnType::CommonName);
    ca_param
        .distinguished_name
        .push(DnType::CommonName, "Comet Self-Signed Certificate");

    ca_param.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    Ok(Certificate::from_params(ca_param)?)
}

fn new_cert(san: &str) -> Result<Certificate> {
    let mut cert_param = CertificateParams::new(vec![san.to_string()]);
    cert_param.distinguished_name.remove(DnType::CommonName);
    cert_param.distinguished_name.push(DnType::CommonName, san);

    Ok(Certificate::from_params(cert_param)?)
}

struct CertResolverInner {
    generated: HashSet<SmolStr>,
    resolver: ResolvesServerCertUsingSNI,
}

impl CertResolverInner {
    fn new() -> Self {
        Self {
            generated: HashSet::new(),
            resolver: ResolvesServerCertUsingSNI::new(),
        }
    }
}

struct CertResolver {
    ca: Certificate,
    ca_rustls: RustlsCert,
    inner: RwLock<CertResolverInner>,
}

impl CertResolver {
    pub fn new() -> Result<Self> {
        let ca = new_ca_cert()?;
        let ca_rustls = RustlsCert(ca.serialize_der()?);

        Ok(Self {
            ca,
            ca_rustls,
            inner: RwLock::new(CertResolverInner::new()),
        })
    }
}

impl ResolvesServerCert for CertResolver {
    fn resolve(
        &self,
        client_hello: tokio_rustls::rustls::ClientHello<'_>,
    ) -> Option<tokio_rustls::rustls::sign::CertifiedKey> {
        let server_name: &str = client_hello.server_name()?.into();

        {
            let inner = self.inner.read().unwrap();
            if inner.generated.contains(server_name) {
                return inner.resolver.resolve(client_hello);
            }
        }

        let cert = new_cert(server_name).unwrap();
        let cert_priv = any_supported_type(&PrivateKey(cert.serialize_private_key_der())).unwrap();
        let cert_pub = RustlsCert(cert.serialize_der_with_signer(&self.ca).unwrap());

        let cert_key =
            CertifiedKey::new(vec![cert_pub, self.ca_rustls.clone()], Arc::new(cert_priv));

        let mut inner = self.inner.write().unwrap();
        inner
            .resolver
            .add(server_name, cert_key)
            .expect("Invalid DNS name");
        inner.resolver.resolve(client_hello)
    }
}

pub struct TlsMitmProcessor {
    acceptor: TlsAcceptor,
}

impl TlsMitmProcessor {
    pub fn new(pipe_tag: &str) -> Result<Self> {
        let mut cfg = ServerConfig::new(NoClientAuth::new());
        cfg.cert_resolver = Arc::new(CertResolver::new()?);

        Ok(TlsMitmProcessor {
            acceptor: TlsAcceptor::from(Arc::new(cfg)),
        })
    }
}

#[async_trait]
impl Processor for TlsMitmProcessor {
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        _conn: &mut Connection,
        _ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        let stream = stream.into_tcp()?;
        let stream = self.acceptor.accept(stream).await?;

        Ok(RWPair::new(stream).into())
    }
}
