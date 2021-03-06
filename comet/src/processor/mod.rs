#[cfg(target_os = "android")]
pub mod android;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod linux;

pub mod any_proxy;
pub mod http;
pub mod set_dest;
pub mod shadowsocks;
pub mod sniffer;
pub mod socks5;
pub mod timeout;
pub mod tls;
#[cfg(feature = "tls-mitm")]
pub mod tls_mitm;
pub mod trojan;
pub mod vmess;
pub mod ws;

use crate::app::plumber::Plumber;

pub fn do_register(plumber: &mut Plumber) {
    #[cfg(target_os = "android")]
    android::register(plumber);
    #[cfg(any(target_os = "linux", target_os = "android"))]
    linux::register(plumber);

    socks5::register(plumber);
    sniffer::register(plumber);
    http::client::register(plumber);
    http::server::register(plumber);
    shadowsocks::register(plumber);
    timeout::register(plumber);
    any_proxy::register(plumber);
    set_dest::register(plumber);
    vmess::register(plumber);
    ws::register(plumber);
    trojan::register(plumber);
    tls::register(plumber);

    #[cfg(feature = "tls-mitm")]
    tls_mitm::register(plumber);
}
