#![cfg(target_os = "android")]
use anyhow::Result;
use jni::objects::{JObject, JClass, JMap, JString};
use jni::sys::jint;
use jni::JNIEnv;
use kokodayo::run_android;
use smol_str::SmolStr;
use std::borrow::Cow;
use std::collections::HashMap;
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
    uid_map: HashMap<u16, SmolStr>,
    stop_receiver: oneshot::Receiver<()>,
) -> Result<()> {
    let running = Arc::new(AtomicBool::new(true));
    run_android(fd, &config_path, uid_map, running.clone()).await?;
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
    uid_map: JObject,
) {
    assert!(STOP_SENDER.is_none());
    init_logger();
    let stop_channel = oneshot::channel();
    STOP_SENDER = Some(stop_channel.0);

    let config_path = env.get_string(config_path).unwrap().into();
    let uid_map = JMap::from_env(&env, uid_map).unwrap();
    let uid_map_rs: Result<HashMap<u16, SmolStr>> = uid_map
        .iter()
        .unwrap()
        .map(|(k, v)| {
            let java_index_value = env.call_method(k, "intValue", "()I", &[])?;
            let index = java_index_value.i()? as u16;
            let java_value = env.get_string(JString::from(v))?;
            Ok((index, Cow::from(&java_value).into()))
        })
        .collect();

    if let Err(error) = start_android(fd as u16, config_path, uid_map_rs.unwrap(), stop_channel.1) {
        let _ = env.throw_new("java/lang/Exception", format!("{}", error));
    }
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_sayori_kokodayo_NativeModule_stop(_env: JNIEnv, _: JClass) {
    if let Some(sender) = STOP_SENDER.take() {
        sender.send(()).unwrap();
    }
}
