use serde::Deserialize;

mod connection;
mod packet;
mod rwpair;

pub use connection::{Connection, DestAddr, UdpRequest};
pub use packet::{AsyncPacketIO, PacketIO};
pub use rwpair::RWPair;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all(deserialize = "snake_case"))]
pub enum TransportType {
    Tcp,
    Udp,
}
