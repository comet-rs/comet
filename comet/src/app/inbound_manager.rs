use crate::config::{Config, Inbound, InboundTransportType};
use crate::prelude::*;
use crate::utils::metered_stream::MeteredStream;
use log::info;
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::io::BufReader;
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::mpsc::{channel, unbounded_channel, Sender, UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;
use tokio_stream::wrappers::ReceiverStream;

pub type ConnSender<T> = UnboundedSender<(Connection, T)>;
pub type ConnReceiver<T> = UnboundedReceiver<(Connection, T)>;

pub struct InboundManager {
    inbounds: HashMap<SmolStr, Inbound>,
    sender: OnceCell<ConnSender<ProxyStream>>,
    udp_table: Mutex<HashMap<SocketAddr, Sender<UdpPacket>>>,
}

impl InboundManager {
    pub fn new(config: &Config) -> Self {
        InboundManager {
            inbounds: config.inbounds.clone(),
            sender: OnceCell::new(),
            udp_table: Mutex::new(HashMap::new()),
        }
    }

    pub async fn start(self: Arc<Self>, ctx: AppContextRef) -> Result<ConnReceiver<ProxyStream>> {
        let channel = unbounded_channel();

        for inbound in &self.inbounds {
            let ctx = ctx.clone();
            let transport = &inbound.1.transport;
            let ip = transport.listen.unwrap_or_else(|| [0, 0, 0, 0].into());
            let port = transport.port;
            let tag = inbound.0.clone();

            match transport.r#type {
                InboundTransportType::Tcp => {
                    let listener = TcpListener::bind(&(ip, port)).await?;
                    let sender = channel.0.clone();
                    info!("Inbound {}/TCP listening on {}:{}", tag, ip, port);

                    let inbound = inbound.1.clone();
                    let manager = self.clone();
                    tokio::spawn(async move {
                        manager
                            .clone()
                            .handle_tcp(listener, tag, inbound, sender, ctx.clone())
                            .await;
                    });
                }
                InboundTransportType::Udp => {
                    let socket = UdpSocket::bind(&(ip, port)).await?;
                    let sender = channel.0.clone();
                    info!("Inbound {}/UDP listening on {}:{}", tag, ip, port);

                    let inbound = inbound.1.clone();
                    let manager = self.clone();
                    tokio::spawn(async move {
                        manager
                            .clone()
                            .handle_udp(socket, tag, inbound, sender, ctx.clone())
                            .await;
                    });
                }
            };
        }

        self.sender.set(channel.0.clone()).unwrap();
        Ok(channel.1)
    }

    async fn handle_tcp(
        self: Arc<Self>,
        listener: TcpListener,
        tag: SmolStr,
        inbound: Inbound,
        sender: ConnSender<ProxyStream>,
        ctx: AppContextRef,
    ) {
        loop {
            let (stream, src_addr) = listener.accept().await.unwrap();
            let conn = Connection::new(
                src_addr,
                tag.clone(),
                inbound.pipeline.clone(),
                TransportType::Tcp,
            );

            info!("Inbound {}/TCP accepted from {}", tag, src_addr);

            let stream = if inbound.metering {
                RWPair::new(MeteredStream::new_inbound(
                    BufReader::new(stream),
                    &tag,
                    &ctx,
                ))
            } else {
                RWPair::new(BufReader::new(stream))
            };

            sender.send((conn, stream.into())).unwrap();
        }
    }

    async fn handle_udp(
        self: Arc<Self>,
        socket: UdpSocket,
        tag: SmolStr,
        inbound: Inbound,
        sender: ConnSender<ProxyStream>,
        _ctx: AppContextRef,
    ) {
        let socket = Arc::new(socket);
        loop {
            let mut buffer = [0u8; 4096];
            let (size, src_addr) = socket.recv_from(&mut buffer).await.unwrap();

            let mut table_ref = self.udp_table.lock().await;

            if let Some(sender) = table_ref.get(&src_addr) {
                let packet = BytesMut::from(&buffer[0..size]);
                if sender.send(UdpPacket::new_unknown(packet)).await.is_ok() {
                    continue;
                }
                // Receiver dropped
            }

            let (read_sender, read_receiver) = channel(10);
            let (write_sender, mut write_receiver) = channel::<UdpPacket>(10);

            let socket_clone = socket.clone();
            let src_addr_clone = src_addr;
            tokio::spawn(async move {
                while let Some(packet) = write_receiver.recv().await {
                    if socket_clone
                        .send_to(&packet, &src_addr_clone)
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            });

            // Insert sender to table to be used later
            table_ref.insert(src_addr, read_sender.clone());
            read_sender
                .send(UdpPacket::new_unknown(BytesMut::from(&buffer[0..size])))
                .await
                .unwrap();

            let conn = Connection::new(
                src_addr,
                tag.clone(),
                inbound.pipeline.clone(),
                TransportType::Udp,
            );
            info!("Inbound {}/UDP accepted from {}", tag, src_addr);

            sender
                .send((
                    conn,
                    UdpStream::new(ReceiverStream::new(read_receiver), write_sender.clone()).into(),
                ))
                .unwrap();
        }
    }

    pub fn inject_tcp(&self) {}
    pub fn inject_udp(&self, tag: &str) -> Result<UdpStream> {
        let (read_sender, read_receiver) = channel(10);
        let (write_sender, write_receiver) = channel(10);

        let mut conn = Connection::new(
            ([0, 0, 0, 0], 0),
            format!("__INTERNAL_{}", tag),
            None,
            TransportType::Udp,
        );

        conn.internal = true;

        self.sender.get().unwrap().send((
            conn,
            UdpStream::new(ReceiverStream::new(read_receiver), write_sender.clone()).into(),
        ))?;

        return Ok(UdpStream::new(
            ReceiverStream::new(write_receiver),
            read_sender,
        ));
    }
}
