use crate::nat::run_router;
use crate::nat_manager::NatManager;
use anyhow::Result;
use jni::objects::JClass;
use jni::sys::jint;
use jni::JNIEnv;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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

#[tokio::main]
pub async fn run_android(fd: u16) -> Result<()> {
    let manager = NatManager::new_ref();
    let ports = start_proxy(Arc::clone(&manager)).await?;
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
    android_logger::init_once(Config::default().with_min_level(Level::Debug));
    let _ = run_android(fd as u16);
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_sayori_kokodayo_NativeModule_stop(_env: JNIEnv, _: JClass) {}
