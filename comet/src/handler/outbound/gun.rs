use super::{NewOutboundHandler, Outbound, OutboundHandler};
use crate::prelude::*;
use anyhow::anyhow;
use std::{
    convert::TryFrom,
    net::{IpAddr, SocketAddr},
};
use tokio_util::io::{ReaderStream, StreamReader};

use tonic::{
    transport::{channel::ClientTlsConfig, Channel, Endpoint, Uri},
    Request,
};
use tower::service_fn;

mod gun_grpc_client {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Hunk {
        #[prost(bytes = "vec", tag = "1")]
        pub data: ::prost::alloc::vec::Vec<u8>,
    }

    use tonic::codegen::*;

    #[derive(Debug, Clone)]
    pub struct GunServiceClient<T> {
        inner: tonic::client::Grpc<T>,
        tun_path: http::uri::PathAndQuery,
        tun_datagram_path: http::uri::PathAndQuery,
    }
    impl GunServiceClient<tonic::transport::Channel> {
        #[doc = r" Attempt to create a new client by connecting to a given endpoint."]
        pub async fn connect<D, N>(dst: D, service_name: N) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
            N: AsRef<str>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn, service_name))
        }
    }

    impl<T> GunServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::ResponseBody: Body + Send + Sync + 'static,
        T::Error: Into<StdError>,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new<N: AsRef<str>>(inner: T, service_name: N) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self {
                inner,
                tun_path: format!("/{}/Tun", service_name.as_ref()).parse().unwrap(),
                tun_datagram_path: format!("/{}/TunDatagram", service_name.as_ref())
                    .parse()
                    .unwrap(),
            }
        }

        pub fn with_interceptor<F, N>(
            inner: T,
            interceptor: F,
            service_name: N,
        ) -> GunServiceClient<InterceptedService<T, F>>
        where
            F: FnMut(tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status>,
            N: AsRef<str>,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<http::Request<tonic::body::BoxBody>>>::Error:
                Into<StdError> + Send + Sync,
        {
            GunServiceClient::new(InterceptedService::new(inner, interceptor), service_name)
        }

        pub async fn tun(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = Hunk>,
        ) -> Result<tonic::Response<tonic::codec::Streaming<Hunk>>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            self.inner
                .streaming(
                    request.into_streaming_request(),
                    self.tun_path.clone(),
                    codec,
                )
                .await
        }

        pub async fn tun_datagram(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = Hunk>,
        ) -> Result<tonic::Response<tonic::codec::Streaming<Hunk>>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            self.inner
                .streaming(
                    request.into_streaming_request(),
                    self.tun_datagram_path.clone(),
                    codec,
                )
                .await
        }
    }
}

use gun_grpc_client::{GunServiceClient, Hunk};

#[derive(Deserialize, Clone, Debug)]
pub struct GunConfig {
    
}

pub struct GunHandler {
    client: GunServiceClient<Channel>,
    metering: bool,
}

impl GunHandler {
    fn create_client(
        addr: IpAddr,
        port: u16,
        service_name: &str,
    ) -> Result<GunServiceClient<Channel>> {
        let connector = move |_: Uri| async move {
            let socket_addr = SocketAddr::from((addr, port));
            crate::net_wrapper::connect_tcp(socket_addr).await
        };

        let channel = Endpoint::try_from("https://[::]:443")?
            .tls_config(ClientTlsConfig::new().domain_name("api-global.reimu.moe"))?
            .connect_with_connector_lazy(service_fn(connector))?;

        Ok(GunServiceClient::new(channel, service_name))
    }
}

#[async_trait]
impl OutboundHandler for GunHandler {
    async fn handle(
        &self,
        tag: &str,
        conn: &mut Connection,
        ctx: &AppContextRef,
    ) -> Result<ProxyStream> {
        let mut client = self.client.clone();
        let (uplink, downlink) = tokio::io::duplex(4096);

        let (uplink_read, mut uplink_write) = tokio::io::split(uplink);
        let reader_stream = ReaderStream::new(uplink_read)
            .take_while(|b| b.is_ok())
            .map(|b| Hunk {
                // Unwrap here is safe because of `take_while`.
                data: b.unwrap().to_vec(),
            });

        let response = client.tun(Request::new(reader_stream)).await?;

        let ss = response.into_inner().map(|h| match h {
            Ok(hunk) => Ok(Bytes::from(hunk.data)),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e)),
        });
        let mut st = StreamReader::new(ss);
        tokio::spawn(async move {
            if let Err(e) = tokio::io::copy(&mut st, &mut uplink_write).await {
                error!("Failed to copy: {}", e);
            }
            let _ = uplink_write.shutdown().await;
        });

        Ok(RWPair::new(downlink).into())
    }
}

impl NewOutboundHandler for GunHandler {
    fn new(config: &Outbound) -> Self {
        Self {
            metering: config.metering,
            client: GunHandler::create_client(IpAddr::from([127, 0, 0, 1]), 443, "mygrpcservice")
                .unwrap(),
        }
    }
}
