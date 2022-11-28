use std::{
    collections::HashSet,
    io::{Read, Write},
    path::Path,
    sync::RwLock,
};

use crate::prelude::*;
use log::{info, warn};
use once_cell::sync::OnceCell;
use rcgen::{BasicConstraints, Certificate, CertificateParams, DnType, IsCa, KeyPair};
use tokio_rustls::{
    rustls::{
        server::{ResolvesServerCert, ResolvesServerCertUsingSni},
        sign::{any_supported_type, CertifiedKey},
        Certificate as RustlsCert, PrivateKey, ServerConfig,
    },
    TlsAcceptor,
};

pub fn register(plumber: &mut Plumber) {
    plumber.register("tls_mitm", |_, pipe_tag| {
        Ok(Box::new(TlsMitmProcessor::new(pipe_tag)?))
    });
}

fn new_cert(san: &str) -> Result<Certificate> {
    let mut cert_param = CertificateParams::new(vec![san.to_string()]);
    cert_param.distinguished_name.remove(DnType::CommonName);
    cert_param.distinguished_name.push(DnType::CommonName, san);

    Ok(Certificate::from_params(cert_param)?)
}

struct CertResolverInner {
    generated: HashSet<SmolStr>,
    resolver: ResolvesServerCertUsingSni,
}

impl CertResolverInner {
    fn new() -> Self {
        Self {
            generated: HashSet::new(),
            resolver: ResolvesServerCertUsingSni::new(),
        }
    }
}

struct CertResolver {
    ca: Certificate,
    ca_rustls: RustlsCert,
    inner: RwLock<CertResolverInner>,
}

impl CertResolver {
    pub fn new(pipe_tag: &str, ctx: AppContextRef) -> Result<Self> {
        let mut ca_path = ctx.data_dir.clone();
        ca_path.push(format!("ca_{}.der", pipe_tag));

        let ca = match Self::parse_cert_file(&ca_path) {
            Ok(ca) => ca,
            Err(e) => {
                warn!("Failed to load CA certificate: {}", e);
                let new_ca = Self::new_ca_cert()?;

                let mut file = std::fs::File::create(&ca_path)?;
                file.write_all(&new_ca.serialize_der()?)?;
                info!("Created new CA certificate at {:?}", ca_path);

                new_ca
            }
        };

        let ca_rustls = RustlsCert(ca.serialize_der()?);

        Ok(Self {
            ca,
            ca_rustls,
            inner: RwLock::new(CertResolverInner::new()),
        })
    }

    fn parse_cert_file(path: &Path) -> Result<Certificate> {
        let mut file = std::fs::File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let kp = KeyPair::from_der(&buffer)?;
        let params = CertificateParams::from_ca_cert_der(&buffer, kp)?;

        Ok(Certificate::from_params(params)?)
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
}

impl ResolvesServerCert for CertResolver {
    fn resolve(
        &self,
        client_hello: tokio_rustls::rustls::server::ClientHello<'_>,
    ) -> Option<Arc<CertifiedKey>> {
        let server_name: &str = client_hello.server_name()?;

        {
            let inner = self.inner.read().unwrap();
            if inner.generated.contains(server_name) {
                return inner.resolver.resolve(client_hello);
            }
        }

        let cert = new_cert(server_name).unwrap();
        let cert_priv = any_supported_type(&PrivateKey(cert.serialize_private_key_der())).unwrap();
        let cert_pub = RustlsCert(cert.serialize_der_with_signer(&self.ca).unwrap());

        let cert_key = CertifiedKey::new(vec![cert_pub, self.ca_rustls.clone()], cert_priv);

        let mut inner = self.inner.write().unwrap();
        inner
            .resolver
            .add(server_name, cert_key)
            .expect("Invalid DNS name");

        info!("Created certificate for {}", server_name);

        inner.resolver.resolve(client_hello)
    }
}

pub struct TlsMitmProcessor {
    pipe_tag: SmolStr,
    acceptor: OnceCell<TlsAcceptor>,
}

impl TlsMitmProcessor {
    pub fn new(pipe_tag: &str) -> Result<Self> {
        Ok(TlsMitmProcessor {
            pipe_tag: pipe_tag.into(),
            acceptor: OnceCell::new(),
        })
    }

    pub fn acceptor(&self, ctx: AppContextRef) -> Result<&TlsAcceptor> {
        self.acceptor.get_or_try_init(|| {
            let cfg = ServerConfig::builder()
                .with_safe_defaults()
                .with_no_client_auth()
                .with_cert_resolver(Arc::new(CertResolver::new(&self.pipe_tag, ctx)?));

            Ok(TlsAcceptor::from(Arc::new(cfg)))
        })
    }
}

#[async_trait]
impl Processor for TlsMitmProcessor {
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        _conn: &mut Connection,
        ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        let stream = stream.into_tcp()?;
        let stream = self.acceptor(ctx)?.accept(stream).await?;

        Ok(RWPair::new(stream).into())
    }
}
