#![cfg(target_os = "android")]
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;

pub mod nat;
pub mod nat_manager;

pub const IPV4_CLIENT: Ipv4Addr = Ipv4Addr::new(10, 25, 1, 1);
pub const IPV4_ROUTER: Ipv4Addr = Ipv4Addr::new(10, 25, 1, 100);
pub const IPV6_CLIENT: Ipv6Addr = Ipv6Addr::new(0xfdfe, 0xdcba, 0x9876, 0, 0, 0, 0, 2);
pub const IPV6_ROUTER: Ipv6Addr = Ipv6Addr::new(0xfdfe, 0xdcba, 0x9876, 0, 0, 0, 0, 1);
