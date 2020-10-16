use crate::nat::run_router;
use crate::nat_manager::NatManager;
use anyhow::Result;
use jni::objects::JClass;
use jni::sys::jint;
use jni::JNIEnv;
use std::net::SocketAddr;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::{TcpListener, UdpSocket};
use tokio::task::spawn_blocking;

use android_logger::Config;
use log::{info, Level};

use crate::proxy::start_proxy;

static RUNNING: AtomicBool = AtomicBool::new(false);

mod nat;
mod nat_manager;
mod proxy;

pub const IPV4_CLIENT: Ipv4Addr = Ipv4Addr::new(10, 25, 1, 1);
pub const IPV4_ROUTER: Ipv4Addr = Ipv4Addr::new(10, 25, 1, 100);
pub const IPV6_CLIENT: Ipv6Addr = Ipv6Addr::new(0xfdfe, 0xdcba, 0x9876, 0, 0, 0, 0, 2);
pub const IPV6_ROUTER: Ipv6Addr = Ipv6Addr::new(0xfdfe, 0xdcba, 0x9876, 0, 0, 0, 0, 1);

#[derive(Clone, Debug)]
pub struct VpnPorts {
    pub tcp_v4: u16,
    pub udp_v4: u16,
    pub dns_v4: u16,
    pub tcp_v6: u16,
    pub udp_v6: u16,
    pub dns_v6: u16,
}

pub struct VpnListeners {
    pub tcp_v4: TcpListener,
    pub udp_v4: UdpSocket,
    pub dns_v4: UdpSocket,
}

impl VpnListeners {
    pub async fn new() -> Result<Self> {
        let addr_v4 = SocketAddr::new(IPV4_CLIENT.into(), 0);
        Ok(VpnListeners {
            tcp_v4: TcpListener::bind(addr_v4).await?,
            udp_v4: UdpSocket::bind(addr_v4).await?,
            dns_v4: UdpSocket::bind(addr_v4).await?,
        })
    }

    pub fn ports(&self) -> VpnPorts {
        VpnPorts {
            tcp_v4: self.tcp_v4.local_addr().unwrap().port(),
            udp_v4: self.udp_v4.local_addr().unwrap().port(),
            dns_v4: self.dns_v4.local_addr().unwrap().port(),
            tcp_v6: 0,
            udp_v6: 0,
            dns_v6: 0,
        }
    }
}

#[tokio::main]
pub async fn run_android(fd: u16) -> Result<()> {
    let manager = NatManager::new_ref();
    let listeners = VpnListeners::new().await?;
    let ports = listeners.ports();

    start_proxy(Arc::clone(&manager), listeners).await?;
    spawn_blocking(move || run_router(fd, Arc::clone(&manager), ports)).await??;
    
    info!("Exiting...");
    Ok(())
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_sayori_kokodayo_NativeModule_start(
    _env: JNIEnv,
    _: JClass,
    fd: jint,
) {
    RUNNING.store(true, Ordering::SeqCst);
    android_logger::init_once(
        Config::default().with_min_level(Level::Debug).with_filter(
            android_logger::FilterBuilder::new()
                .parse("debug,trust_dns_proto=error")
                .build(),
        ),
    );
    let _ = run_android(fd as u16);
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_sayori_kokodayo_NativeModule_stop(_env: JNIEnv, _: JClass) {}
