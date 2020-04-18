
use std::net::Ipv6Addr;
use std::net::Ipv4Addr;
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{jstring, jint};

use android_logger::Config;
use log::{Level, info};

use std::os::raw::{c_char};
use std::ffi::{CString, CStr};

mod nat_manager;
mod nat;
mod proxy;
use nat::run_android;

pub const IPV4_CLIENT: Ipv4Addr = Ipv4Addr::new(10, 25, 1, 1);
pub const IPV4_ROUTER: Ipv4Addr = Ipv4Addr::new(10, 25, 1, 100);
pub const IPV6_CLIENT: Ipv6Addr = Ipv6Addr::new(0xfdfe, 0xdcba, 0x9876, 0, 0, 0, 0, 2);
pub const IPV6_ROUTER: Ipv6Addr = Ipv6Addr::new(0xfdfe, 0xdcba, 0x9876, 0, 0, 0, 0, 1);

#[no_mangle]
pub extern fn rust_greeting(to: *const c_char) -> *mut c_char {
    let c_str = unsafe { CStr::from_ptr(to) };
    let recipient = match c_str.to_str() {
        Err(_) => "there",
        Ok(string) => string,
    };

    CString::new("Hello ".to_owned() + recipient).unwrap().into_raw()
}

#[no_mangle]
pub unsafe extern fn Java_com_sayori_kokodayo_NativeModule_greeting(env: JNIEnv, _: JClass, java_pattern: JString) -> jstring {
    // Our Java companion code might pass-in "world" as a string, hence the name.
    let world = rust_greeting(env.get_string(java_pattern).expect("invalid pattern string").as_ptr());
    // Retake pointer so that we can use it below and allow memory to be freed when it goes out of scope.
    let world_ptr = CString::from_raw(world);
    let output = env.new_string(world_ptr.to_str().unwrap()).expect("Couldn't create java string!");

    output.into_inner()
}

#[no_mangle]
pub unsafe extern fn Java_com_sayori_kokodayo_NativeModule_start(env: JNIEnv, _: JClass, fd: jint) -> jstring {
    // Retake pointer so that we can use it below and allow memory to be freed when it goes out of scope.
    let world_ptr = CString::new("Done").unwrap();
    let output = env.new_string(world_ptr.to_str().unwrap()).expect("Couldn't create java string!");

    android_logger::init_once(
        Config::default().with_min_level(Level::Trace),
    );

    let utils_class = env.find_class("android/net/NetworkUtils");
    info!("Utils class: {:?}", utils_class);

    let _ = run_android(fd as u16);

    output.into_inner()
}