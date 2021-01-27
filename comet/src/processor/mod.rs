#[cfg(target_os = "android")]
pub mod android;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod linux;

pub mod any_proxy;
pub mod http;
pub mod shadowsocks;
pub mod sniffer;
pub mod socks5_proxy;
pub mod switch;
pub mod timeout;
pub mod tls_mitm;

use crate::app::plumber::Plumber;

pub fn do_register(plumber: &mut Plumber) {
    #[cfg(target_os = "android")]
    android::register(plumber);
    #[cfg(any(target_os = "linux", target_os = "android"))]
    linux::register(plumber);

    socks5_proxy::register(plumber);
    sniffer::register(plumber);
    http::client::register(plumber);
    http::server::register(plumber);
    shadowsocks::register(plumber);
    timeout::register(plumber);
    any_proxy::register(plumber);
}
