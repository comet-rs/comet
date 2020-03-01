use anyhow::{anyhow, Result};
use log::info;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use tokio::net::UdpSocket;
use trust_dns_proto::op::header::MessageType;
use trust_dns_proto::op::message::Message;

pub async fn process_query(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut message = Message::from_vec(bytes)?;
    message.set_message_type(MessageType::Response);
    message.set_authoritative(false);
    message.set_recursion_available(true);
    message.set_authentic_data(false);
    info!("DNS request: {:?}", message);

    let mut out_sock =
        UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0)).await?;
    out_sock.connect((Ipv4Addr::new(1,2,4,8), 53)).await?;
    out_sock.send(bytes).await?;

    let mut buffer = vec![0; 512];
    let size = out_sock.recv(&mut buffer[..]).await?;
    buffer.resize(size, 0);
    let resp_message = Message::from_vec(&buffer[..])?;
    info!("DNS response: {:?}", resp_message);
    Ok(buffer)
}
