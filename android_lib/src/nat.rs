use crate::proxy::ProxyPorts;
use crate::{IPV4_CLIENT, IPV4_ROUTER, IPV6_CLIENT, IPV6_ROUTER};
use anyhow::{anyhow, Result};
use log::{error, info};
use pnet::packet;
use pnet::packet::tcp::MutableTcpPacket;
use pnet::packet::udp::MutableUdpPacket;
use std::net::IpAddr;
use std::os::unix::io::FromRawFd;
use std::os::unix::io::RawFd;

use nix::sys::select::{select, FdSet};
use nix::unistd;
use pnet::packet::ip::*;
use pnet::packet::MutablePacket;
use std::collections::VecDeque;

use crate::nat_manager::{NatManagerRef, ProtocolType};

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
    fin: bool,
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
            fin: raw & FIN != 0,
        }
    }
}

#[derive(Debug)]
struct AddressedPacket<T> {
    pub src_addr: IpAddr,
    pub dest_addr: IpAddr,
    pub inner: T,
}

impl<T> AddressedPacket<T> {
    pub fn is_from_client(&self) -> bool {
        self.src_addr == IpAddr::V4(IPV4_CLIENT) || self.src_addr == IpAddr::V6(IPV6_CLIENT)
    }

    pub fn is_to_router(&self) -> bool {
        self.dest_addr == IpAddr::V4(IPV4_ROUTER) || self.dest_addr == IpAddr::V6(IPV6_ROUTER)
    }
}

type AddressedTcpPacket<'p> = AddressedPacket<MutableTcpPacket<'p>>;
type AddressedUdpPacket<'p> = AddressedPacket<MutableUdpPacket<'p>>;

fn handle_tcp(
    manager: &mut NatManagerRef,
    packet: &mut AddressedTcpPacket<'_>,
    ports: &ProxyPorts,
) -> Result<()> {
    let flags = TcpFlags::new(packet.inner.get_flags());
    // trace!("Got TCP packet: [{:?}] {:?}", flags, packet);
    if packet.is_from_client() {
        if packet.is_to_router() {
            // Return packet to orig
            if let Some((dest_addr, dest_port)) = manager.get_entry(
                ProtocolType::Tcp,
                packet.inner.get_destination(),
                packet.dest_addr,
            ) {
                packet.src_addr = dest_addr;
                packet.dest_addr = match dest_addr {
                    IpAddr::V4(_) => IpAddr::V4(IPV4_CLIENT),
                    IpAddr::V6(_) => IpAddr::V6(IPV6_CLIENT),
                };
                packet.inner.set_source(dest_port);
            } else {
                return Err(anyhow!("Entry not found in NAT table"));
            }
        } else {
            // Forward packet to proxy
            if flags.syn && !flags.ack {
                info!(
                    "New TCP conn: {}:{}",
                    packet.dest_addr,
                    packet.inner.get_destination()
                );
                manager.new_entry(
                    ProtocolType::Tcp,
                    packet.inner.get_source(),
                    packet.dest_addr,
                    packet.inner.get_destination(),
                );
            } else {
                let refresh_result = manager.refresh_entry(
                    ProtocolType::Tcp,
                    packet.inner.get_source(),
                    packet.dest_addr,
                    packet.inner.get_destination(),
                );
                if !refresh_result {
                    return Err(anyhow!("Entry not found in NAT table"));
                }
            }
            match packet.src_addr {
                IpAddr::V4(_) => {
                    packet.src_addr = IpAddr::V4(IPV4_ROUTER);
                    packet.dest_addr = IpAddr::V4(IPV4_CLIENT);
                    packet.inner.set_destination(ports.tcp_v4);
                }
                IpAddr::V6(_) => {
                    packet.src_addr = IpAddr::V6(IPV6_ROUTER);
                    packet.dest_addr = IpAddr::V6(IPV6_CLIENT);
                    packet.inner.set_destination(ports.tcp_v6);
                }
            };
        }
    } else {
        return Err(anyhow!("Unknown source address: {}", packet.src_addr));
    }
    Ok(())
}

fn handle_udp(
    manager: &mut NatManagerRef,
    packet: &mut AddressedUdpPacket<'_>,
    ports: &ProxyPorts,
) -> Result<()> {
    if packet.is_from_client() {
        if packet.is_to_router() {
            // Return packet to orig
            if let Some((dest_addr, dest_port)) = manager.get_entry(
                ProtocolType::Udp,
                packet.inner.get_destination(),
                packet.dest_addr,
            ) {
                packet.src_addr = dest_addr;
                packet.dest_addr = match dest_addr {
                    IpAddr::V4(_) => IpAddr::V4(IPV4_CLIENT),
                    IpAddr::V6(_) => IpAddr::V6(IPV6_CLIENT),
                };
                packet.inner.set_source(dest_port);
            } else {
                return Err(anyhow!("Entry not found in NAT table"));
            }
        } else {
            // Forward packet to proxy
            let refresh_result = manager.refresh_entry(
                ProtocolType::Udp,
                packet.inner.get_source(),
                packet.dest_addr,
                packet.inner.get_destination(),
            );
            if !refresh_result {
                manager.new_entry(
                    ProtocolType::Udp,
                    packet.inner.get_source(),
                    packet.dest_addr,
                    packet.inner.get_destination(),
                );
            }

            match packet.src_addr {
                IpAddr::V4(_) => {
                    packet.src_addr = IpAddr::V4(IPV4_ROUTER);
                    packet.dest_addr = IpAddr::V4(IPV4_CLIENT);
                    if packet.inner.get_destination() == 53 {
                        packet.inner.set_destination(ports.dns_v4);
                    } else {
                        packet.inner.set_destination(ports.udp_v4);
                    }
                }
                IpAddr::V6(_) => {
                    packet.src_addr = IpAddr::V6(IPV6_ROUTER);
                    packet.dest_addr = IpAddr::V6(IPV6_CLIENT);
                    if packet.inner.get_destination() == 53 {
                        packet.inner.set_destination(ports.dns_v6);
                    } else {
                        packet.inner.set_destination(ports.udp_v6);
                    }
                }
            };
        }
    } else {
        return Err(anyhow!("Unknown source address: {}", packet.src_addr));
    }
    Ok(())
}

fn handle_ipv4(manager: &mut NatManagerRef, buffer: &mut [u8], ports: &ProxyPorts) -> Result<()> {
    let mut ip_pkt = packet::ipv4::MutableIpv4Packet::new(buffer)
        .ok_or(anyhow!("Failed to parse IPv4 packet"))?;
    let l4_proto = ip_pkt.get_next_level_protocol();

    let mut src_addr = ip_pkt.get_source();
    let mut dest_addr = ip_pkt.get_destination();

    match l4_proto {
        IpNextHeaderProtocols::Tcp => {
            use pnet::packet::tcp::ipv4_checksum;
            let tcp_pkt = packet::tcp::MutableTcpPacket::new(ip_pkt.payload_mut())
                .ok_or(anyhow!("Failed to parse TCP packet"))?;

            let mut addressed = AddressedPacket {
                src_addr: IpAddr::V4(src_addr),
                dest_addr: IpAddr::V4(dest_addr),
                inner: tcp_pkt,
            };
            handle_tcp(manager, &mut addressed, &ports)?;
            src_addr = match addressed.src_addr {
                IpAddr::V4(addr) => addr,
                IpAddr::V6(_) => unreachable!(),
            };
            dest_addr = match addressed.dest_addr {
                IpAddr::V4(addr) => addr,
                IpAddr::V6(_) => unreachable!(),
            };
            addressed.inner.set_checksum(ipv4_checksum(
                &addressed.inner.to_immutable(),
                &src_addr,
                &dest_addr,
            ));
        }
        IpNextHeaderProtocols::Udp => {
            use pnet::packet::udp::ipv4_checksum;
            let udp_pkt = MutableUdpPacket::new(ip_pkt.payload_mut())
                .ok_or(anyhow!("Failed to parse UDP packet"))?;

            let mut addressed = AddressedPacket {
                src_addr: IpAddr::V4(src_addr),
                dest_addr: IpAddr::V4(dest_addr),
                inner: udp_pkt,
            };

            handle_udp(manager, &mut addressed, &ports)?;
            src_addr = match addressed.src_addr {
                IpAddr::V4(addr) => addr,
                IpAddr::V6(_) => unreachable!(),
            };
            dest_addr = match addressed.dest_addr {
                IpAddr::V4(addr) => addr,
                IpAddr::V6(_) => unreachable!(),
            };
            addressed.inner.set_checksum(ipv4_checksum(
                &addressed.inner.to_immutable(),
                &src_addr,
                &dest_addr,
            ));
        }
        _ => {
            return Err(anyhow!("Unsupported protocol: {:?}", l4_proto));
        }
    };
    ip_pkt.set_source(src_addr);
    ip_pkt.set_destination(dest_addr);

    {
        use pnet::packet::ipv4::checksum;
        ip_pkt.set_checksum(checksum(&ip_pkt.to_immutable()));
    }

    Ok(())
}

fn select_fds(
    mut read_set: FdSet,
    mut write_set: FdSet,
    mut error_set: FdSet,
) -> Result<(FdSet, FdSet)> {
    select(None, &mut read_set, &mut write_set, &mut error_set, None)?;
    Ok((read_set, write_set))
}

pub fn run_router(fd: u16, mut manager: NatManagerRef, ports: ProxyPorts) -> Result<()> {
    let raw_fd = fd as RawFd;
    const QUEUE_CAP: usize = 10;
    let _file = unsafe { std::fs::File::from_raw_fd(raw_fd) };

    let mut read_set = FdSet::new();
    let mut write_set = FdSet::new();
    let mut error_set = FdSet::new();
    let mut write_queue: VecDeque<Vec<u8>> = VecDeque::with_capacity(QUEUE_CAP);

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

        error_set.insert(fd as RawFd);

        let (mut read_set, mut write_set) = select_fds(read_set, write_set, error_set)?;

        if error_set.contains(raw_fd) {
            unistd::read(raw_fd, &mut [])?;
        }

        if read_set.contains(raw_fd) {
            // Reading available
            let mut buffer = vec![0; 1500];
            let n = unistd::read(raw_fd, &mut buffer[..])?;

            let handle_result = match buffer[0] >> 4 {
                4 => handle_ipv4(&mut manager, &mut buffer[0..n], &ports),
                _ => continue,
            };
            match handle_result {
                Ok(_) => {
                    buffer.resize(n, 0);
                    write_queue.push_back(buffer);
                }
                Err(e) => error!("Packet handle failed: {:?}", e),
            }
        }

        if write_set.contains(raw_fd) {
            // Writing available
            let buffer = write_queue.pop_front().unwrap();
            unistd::write(raw_fd, &buffer)?;
        }
    }
}
