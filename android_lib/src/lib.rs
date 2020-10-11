use jni::objects::JClass;
use jni::sys::{jint, jstring};
use jni::JNIEnv;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;

use android_logger::Config;
use log::{info, Level};

use std::ffi::CString;

mod nat;
mod nat_manager;
mod proxy;
use nat::run_android;

pub const IPV4_CLIENT: Ipv4Addr = Ipv4Addr::new(10, 25, 1, 1);
pub const IPV4_ROUTER: Ipv4Addr = Ipv4Addr::new(10, 25, 1, 100);
pub const IPV6_CLIENT: Ipv6Addr = Ipv6Addr::new(0xfdfe, 0xdcba, 0x9876, 0, 0, 0, 0, 2);
pub const IPV6_ROUTER: Ipv6Addr = Ipv6Addr::new(0xfdfe, 0xdcba, 0x9876, 0, 0, 0, 0, 1);

#[no_mangle]
pub unsafe extern "C" fn Java_com_sayori_kokodayo_NativeModule_start(
    env: JNIEnv,
    _: JClass,
    fd: jint,
) -> jstring {
    // Retake pointer so that we can use it below and allow memory to be freed when it goes out of scope.
    let world_ptr = CString::new("Done").unwrap();
    let output = env
        .new_string(world_ptr.to_str().unwrap())
        .expect("Couldn't create java string!");

    android_logger::init_once(Config::default().with_min_level(Level::Debug));

    let utils_class = env.find_class("android/net/NetworkUtils");
    info!("Utils class: {:?}", utils_class);

    let _ = run_android(fd as u16);

    output.into_inner()
}
