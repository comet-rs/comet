use std::{collections::HashSet, sync::RwLock};

use crate::prelude::*;
use rcgen::{BasicConstraints, Certificate, CertificateParams, DnType, IsCa};
use tokio::sync::Mutex;
use tokio_rustls::rustls::{
    Certificate as RustlsCert, ResolvesServerCert, ResolvesServerCertUsingSNI,
};

pub fn register(plumber: &mut Plumber) {
    plumber.register("tls_mitm", |_| Ok(Box::new(TlsMitmProcessor {})));
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
    let cert_param = CertificateParams::new(vec![san.to_string()]);
    Ok(Certificate::from_params(cert_param)?)
}

struct CertResolver {
    ca: Certificate,
    ca_rustls: RustlsCert,
    generated: HashSet<SmolStr>,
    inner: RwLock<ResolvesServerCertUsingSNI>,
}

impl CertResolver {
    pub fn new() -> Result<Self> {
        let ca = new_ca_cert()?;
        let ca_rustls = RustlsCert(ca.serialize_der()?);

        Ok(Self {
            ca,
            ca_rustls,
            generated: HashSet::new(),
            inner: RwLock::new(ResolvesServerCertUsingSNI::new()),
        })
    }
}

impl ResolvesServerCert for CertResolver {
    fn resolve(
        &self,
        client_hello: tokio_rustls::rustls::ClientHello<'_>,
    ) -> Option<tokio_rustls::rustls::sign::CertifiedKey> {
        let server_name: &str = client_hello.server_name()?.into();
        if self.generated.contains(server_name) {
            return self.inner.resolve(client_hello);
        }

        let cert = new_cert(server_name).unwrap();
        let cert_pub = cert.serialize_der_with_signer(&self.ca).unwrap();
        None
    }
}

pub struct TlsMitmProcessor {}

#[async_trait]
impl Processor for TlsMitmProcessor {
    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        conn: &mut Connection,
        _ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        todo!()
    }
}
