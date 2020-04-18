use crate::IncomingPacketRawTransport;
use crate::IncomingStreamRawTransport;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use log::error;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct IncomingRawTransportManager {
    stream: Vec<(String, Box<dyn IncomingStreamRawTransport + Send + Sync>)>,
    packet: Vec<(String, Box<dyn IncomingPacketRawTransport + Send + Sync>)>,
}

impl IncomingRawTransportManager {
    pub fn new() -> Self {
        Self {
            stream: Vec::new(),
            packet: Vec::new(),
        }
    }

    pub fn add_stream<T: IncomingStreamRawTransport + Send + Sync + 'static>(
        &mut self,
        transport: T,
        protocol_tag: String,
    ) {
        self.stream.push((protocol_tag, Box::new(transport)));
    }

    pub fn add_packet<T: IncomingPacketRawTransport + Send + Sync + 'static>(
        &mut self,
        transport: T,
        protocol_tag: String,
    ) {
        self.packet.push((protocol_tag, Box::new(transport)));
    }

    pub fn start(&mut self) {
        while let Some((proto_tag, tp)) = self.stream.pop() {
            let (sender, receiver) = unbounded();

            tokio::spawn(async move {
                let exit_status = tp.start(sender).await;
                if let Err(error) = exit_status {
                    error!("Listener exited with error: {:?}", error);
                }
            });
        }
    }
}

pub struct OutgoingRawTransportManager {}
