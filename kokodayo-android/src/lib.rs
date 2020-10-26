use anyhow::Result;
use jni::objects::{JString, JClass, JMap};
use jni::sys::jint;
use jni::JNIEnv;
use kokodayo::run_android;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::oneshot;

use android_logger::Config;
use log::{info, Level};

static mut STOP_SENDER: Option<oneshot::Sender<()>> = None;

#[tokio::main]
pub async fn start_android(
    fd: u16,
    config_path: String,
    stop_receiver: oneshot::Receiver<()>,
) -> Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    run_android(fd, &config_path, running.clone()).await?;
    info!("Service started, call `stop` to stop");
    stop_receiver.await?;
    info!("Stopping...");
    running.store(false, Ordering::Relaxed);

    Ok(())
}

fn init_logger() {
    android_logger::init_once(
        Config::default().with_min_level(Level::Debug).with_filter(
            android_logger::FilterBuilder::new()
                .parse("debug,trust_dns_proto=error")
                .build(),
        ),
    );
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_sayori_kokodayo_NativeModule_start(
    env: JNIEnv,
    _: JClass,
    fd: jint,
    config_path: JString,
) {
    assert!(STOP_SENDER.is_none());
    init_logger();
    let stop_channel = oneshot::channel();
    STOP_SENDER = Some(stop_channel.0);
    let config_path = env.get_string(config_path).unwrap().into();
    if let Err(error) = start_android(fd as u16, config_path, stop_channel.1) {
        let _ = env.throw_new("java/lang/Exception", format!("{}", error));
    }
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_sayori_kokodayo_NativeModule_stop(_env: JNIEnv, _: JClass) {
    if let Some(sender) = STOP_SENDER.take() {
        sender.send(()).unwrap();
    }
}
