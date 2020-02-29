use std::os::unix::io::FromRawFd;
use std::os::unix::io::RawFd;
use anyhow::{Result, anyhow};
use log::{info, error, trace};
use pnet::packet;

use nix::sys::select::{select, FdSet};
use nix::unistd;
use std::collections::VecDeque;
use pnet::packet::ip::*;
use pnet::packet::{Packet, MutablePacket};

use crate::nat_manager::NatManager;

#[derive(Debug)]
enum IPPacket<'a> {
    V4(packet::ipv4::MutableIpv4Packet<'a>),
    V6(packet::ipv6::MutableIpv6Packet<'a>)
}

impl<'a> IPPacket<'a> {
    pub fn get_next_level_protocol(&self) -> IpNextHeaderProtocol {
        match self {
            IPPacket::V4(ref pkt) => pkt.get_next_level_protocol(),
            IPPacket::V6(ref pkt) => pkt.get_next_header(),
        }
    }
}

impl<'a> Packet for IPPacket<'a> {
    fn packet<'p>(&'p self) -> &'p [u8] {
        match self {
            IPPacket::V4(ref pkt) => pkt.packet(),
            IPPacket::V6(ref pkt) => pkt.packet(),
        }
    }
    fn payload<'p>(&'p self) -> &'p [u8] {
        match self {
            IPPacket::V4(ref pkt) => pkt.payload(),
            IPPacket::V6(ref pkt) => pkt.payload(),
        }
    }
}

impl<'a> MutablePacket for IPPacket<'a> {
    fn packet_mut<'p>(&'p mut self) -> &'p mut [u8] {
        match self {
            IPPacket::V4(ref mut pkt) => pkt.packet_mut(),
            IPPacket::V6(ref mut pkt) => pkt.packet_mut(),
        }
    }

    fn payload_mut<'p>(&'p mut self) -> &'p mut [u8] {
        match self {
            IPPacket::V4(ref mut pkt) => pkt.payload_mut(),
            IPPacket::V6(ref mut pkt) => pkt.payload_mut(),
        }
    }

    fn clone_from<T: packet::Packet>(&mut self, other: &T) {
        match self {
            IPPacket::V4(ref mut pkt) => pkt.clone_from(other),
            IPPacket::V6(ref mut pkt) => pkt.clone_from(other),
        }
    }
}

#[derive(Debug)]
struct TcpFlags {
    ns: bool,
    cwr: bool,
    ece: bool,
    urg: bool,
    ack: bool,
    psh: bool,
    rst: bool,
    syn: bool,
    fin: bool
}

impl TcpFlags {
    pub fn new(raw: u16) -> TcpFlags {
        use pnet::packet::tcp::TcpFlags::*;
        TcpFlags {
            ns: raw & NS != 0,
            cwr: raw & CWR != 0,
            ece: raw & ECE != 0,
            urg: raw & URG != 0,
            ack: raw & ACK != 0,
            psh: raw & PSH != 0,
            rst: raw & RST != 0,
            syn: raw & SYN != 0,
            fin: raw & FIN != 0
        }
    }
}

fn handle_tcp<'p>(mut ip_pkt: IPPacket<'p>) -> Result<IPPacket<'p>> {
    let tcp_pkt = packet::tcp::MutableTcpPacket::new(ip_pkt.payload_mut()).ok_or(anyhow!("Failed to parse TCP packet"))?;
    trace!("TCP packet: {:?}", tcp_pkt);
    trace!("TCP flags: {:?}", TcpFlags::new(tcp_pkt.get_flags()));
    Ok(ip_pkt)
}

fn run_router(fd: u16) -> Result<()> {
    let raw_fd = fd as RawFd;
    const QUEUE_CAP: usize = 10;
    let file = unsafe { std::fs::File::from_raw_fd(raw_fd) };
    

    let mut read_set = FdSet::new();
    let mut write_set = FdSet::new();
    let write_queue: VecDeque<Vec<u8>> = VecDeque::with_capacity(QUEUE_CAP);

    loop {
        let qlen = write_queue.len();
        read_set.clear();
        write_set.clear();
        if qlen < QUEUE_CAP {
            read_set.insert(fd as RawFd);
        }
        if qlen > 0 {
            write_set.insert(fd as RawFd);
        }

        select(None, &mut read_set, &mut write_set, None, None)?;

        if read_set.contains(raw_fd) { // Reading available
            let mut buffer = vec![0; 1500];
            let n = unistd::read(raw_fd, &mut buffer[..])?;
            let pkt = match buffer[0] >> 4 {
                4 => packet::ipv4::MutableIpv4Packet::new(&mut buffer[0..n]).map(|p| IPPacket::V4(p)),
                6 => packet::ipv6::MutableIpv6Packet::new(&mut buffer[0..n]).map(|p| IPPacket::V6(p)),
                _ => continue
            };

            if pkt.is_none() {
                continue
            }
            let packet = pkt.unwrap();
            let next_level = packet.get_next_level_protocol();

            trace!("Packet Len = {}, [{}], {:?}", n, next_level, packet);
            let processed = match next_level {
                IpNextHeaderProtocols::Tcp => handle_tcp(packet),
                _ => continue
            };

        }
    }
}

pub fn run_android(fd: u16) {
    let manager = NatManager::new();
    error!("NAT thread exited: {:?}", run_router(fd));
}
