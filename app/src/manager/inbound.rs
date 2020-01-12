use anyhow::Result;
use common::protocol::InboundProtocol;
use settings::inbound::InboundSettings;

pub struct InboundManager {
    items: Vec<InboundItem>,
}

pub struct InboundItem {
    protocol: Box<dyn InboundProtocol>,
}

impl InboundManager {
    pub fn new(settings: Vec<InboundSettings>) -> Result<InboundManager> {
        use settings::inbound::InboundProtocolType;
        use socks5::InboundSocks5Protocol;
        let mut inbounds = Vec::with_capacity(settings.len());

        for set in settings {
            let proto = match set.protocol {
                InboundProtocolType::Socks(s) => InboundSocks5Protocol::new(&s)?,
            };
            inbounds.push(InboundItem {
                protocol: Box::new(proto),
            });
        }

        Ok(InboundManager { items: inbounds })
    }
}
