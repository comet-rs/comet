pub mod app;
pub mod common;
pub mod config;
pub mod crypto;
pub mod dns;
pub mod handler;
pub mod net_wrapper;
pub mod prelude;
pub mod processor;
pub mod router;
pub mod utils;
pub mod protos;
pub mod rule_provider;

#[cfg(target_os = "android")]
pub mod android;

use crate::app::dispatcher;
use crate::prelude::*;

use anyhow::Context;

pub async fn run(ctx: AppContextRef) -> Result<()> {
    let mut conns = ctx.clone_inbound_manager().start(ctx.clone()).await?;
    ctx.dns.start(ctx.clone()).await?;

    let ctx_tcp = ctx.clone();
    let _process_handle = tokio::spawn(async move {
        while let Some((mut conn, stream)) = conns.recv().await {
            let ctx = ctx_tcp.clone();
            tokio::spawn(async move {
                match dispatcher::handle_conn(&mut conn, stream, ctx).await {
                    Ok(()) => {
                        info!("Finished handling {}", conn);
                    }
                    Err(err) => {
                        let cause: Vec<_> = err.chain().skip(1).map(|c| format!("{}", c)).collect();
                        error!(
                            "Failed to handle {} because {} > {}",
                            conn,
                            err,
                            cause.join(" > ")
                        );
                    }
                }
            });
        }
    });

    Ok(())
}

pub async fn run_bin() -> Result<()> {
    let config = config::load_file("./config.yml")
        .await
        .context("Failed to read config file")?;

    println!("{:#?}", config);
    let ctx = Arc::new(AppContext::new(&config)?);
    drop(config);

    run(ctx).await?;
    Ok(())
}

#[cfg(target_os = "android")]
pub async fn run_android(
    fd: u16,
    config_path: &str,
    uid_map: HashMap<u32, SmolStr>,
    running: Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    info!("{:?}", uid_map);

    let config = config::load_file(config_path)
        .await
        .context("Failed to read config file")?;
    let ctx = Arc::new(AppContext::new(&config)?);
    drop(config);

    let ctx1 = ctx.clone();
    std::thread::spawn(move || match android::nat::run_router(fd, ctx1, running) {
        Ok(_) => info!("Android router exited"),
        Err(err) => error!("Android router failed: {}", err),
    });

    run(ctx).await?;
    Ok(())
}
