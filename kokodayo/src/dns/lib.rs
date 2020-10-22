use anyhow::{anyhow, Result};
use log::{debug, info};
use net_wrapper::bind_udp;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use trust_dns_client::rr::Record;

use trust_dns_client::rr::{Name, RecordType};
use trust_dns_client::serialize::binary::{BinDecodable, BinEncodable};
use trust_dns_proto::op::{Message, MessageType, OpCode, Query};

const MAX_PAYLOAD_LEN: u16 = 1500 - 40 - 8;

pub struct DnsService {
    cache: HashMap<Query, Record>,
}

impl DnsService {}

fn new_lookup(name: Name, query_type: RecordType) -> Message {
    info!("querying: {} {:?}", name, query_type);

    let query = Query::query(name, query_type);
    let mut message: Message = Message::new();

    // TODO: This is not the final ID, it's actually set in the poll method of DNS future
    //  should we just remove this?
    let id: u16 = rand::random();

    message.add_query(query);
    message
        .set_id(id)
        .set_message_type(MessageType::Query)
        .set_op_code(OpCode::Query)
        .set_recursion_desired(true);

    // Extended dns
    {
        // TODO: this should really be configurable...
        let edns = message.edns_mut();
        edns.set_max_payload(MAX_PAYLOAD_LEN);
        edns.set_version(0);
    }

    message
}

async fn xfer_message(query: Message) -> Result<Message> {
    let message_raw = query.to_bytes()?;

    let mut out_sock = bind_udp(&SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0)).await?;
    out_sock.connect((Ipv4Addr::new(1, 2, 4, 8), 53)).await?;
    out_sock.send(&message_raw[..]).await?;

    let mut buffer = [0u8; 512];
    let size = out_sock.recv(&mut buffer[..]).await?;

    Ok(Message::from_vec(&buffer[0..size])?)
}

pub async fn process_query(bytes: &[u8]) -> Result<Vec<u8>> {
    let message_query = Message::from_vec(bytes)?;
    let id = message_query.id();

    if message_query.query_count() < 1 {
        return Err(anyhow!("No query found in DNS request"));
    }

    let query = &message_query.queries()[0];

    let upstream_query = new_lookup(query.name().clone(), query.query_type());
    let mut upstream_response = xfer_message(upstream_query).await?;
    upstream_response.set_id(id);
    // debug!("DNS response: {:?}", upstream_response);

    Ok(upstream_response.to_vec()?)
}
