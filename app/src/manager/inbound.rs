use anyhow::Result;
use common::connection::AcceptedConnection;
use common::connection::InboundConnection;
use common::protocol::InboundProtocol;
use futures::channel::mpsc;
use futures::future::FutureExt;
use futures::sink::SinkExt;
use log::{error, info};
use settings::inbound::InboundSettings;
use std::sync::Arc;
use transport::inbound::InboundTransport;

pub struct InboundManager {
    items: Vec<InboundItem>,
}

pub struct InboundItem {
    protocol: Box<dyn InboundProtocol + Send>,
    settings: InboundSettings,
}

impl InboundManager {
    pub fn new(settings: Vec<InboundSettings>) -> Result<InboundManager> {
        use settings::inbound::InboundProtocolType;
        use socks5::InboundSocks5Protocol;
        let mut inbounds = Vec::with_capacity(settings.len());

        for set in settings {
            let proto = match set.protocol {
                InboundProtocolType::Socks(ref s) => InboundSocks5Protocol::new(s)?,
            };
            inbounds.push(InboundItem {
                settings: set,
                protocol: Box::new(proto),
            });
        }

        Ok(InboundManager { items: inbounds })
    }

    pub async fn run(self) -> Result<mpsc::Receiver<AcceptedConnection>> {
        let (sender, receiver) = mpsc::channel::<AcceptedConnection>(self.items.len());
        for item in self.items {
            let transport = transport::inbound::create_transport(&item.settings).await?;
            tokio::spawn(acceptor(transport, item, sender.clone()));
        }
        Ok(receiver)
    }
}

async fn acceptor(
    mut transport: Box<dyn InboundTransport>,
    inbound: InboundItem,
    sink: mpsc::Sender<AcceptedConnection>,
) -> Result<()> {
    let ib_arc = Arc::new(inbound);
    loop {
        let conn = transport.accept().await?;
        let ib = ib_arc.clone();
        tokio::spawn(accept_conn(conn, ib, sink.clone()).map(|r| {
            if let Err(e) = r {
                error!("Failed to handle connection: {}", e);
            }
        }));
    }
}

async fn accept_conn(
    conn: InboundConnection,
    inbound: Arc<InboundItem>,
    mut sink: mpsc::Sender<AcceptedConnection>,
) -> Result<()> {
    let mut handled = inbound.protocol.accept(conn).await?;
    if inbound.settings.sniffing.enabled {
        let (cached_payload, sniff_result) = transport::sniff(&mut handled.conn).await?;
        handled.sniffer_data = Some(cached_payload);
        handled.sniffed_dest = sniff_result;
    }
    info!("Handled {:?}", handled);
    sink.send(handled).await?;
    Ok(())
}
