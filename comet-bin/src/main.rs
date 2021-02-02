use anyhow::Result;
use clap::Clap;
use comet::{run_bin, CONN_ID};
use fern::colors::{Color, ColoredLevelConfig};
use log::{info, LevelFilter};
use tokio::signal;

fn setup_logger(level: LevelFilter) -> Result<(), fern::InitError> {
    let colors_level = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Green)
        .debug(Color::BrightBlue)
        .trace(Color::BrightBlack);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            let date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let level = colors_level.color(record.level());
            let conn_id = CONN_ID.try_with(|id| *id).ok();
            let target = record.target();

            if let Some(conn_id) = conn_id {
                out.finish(format_args!(
                    "[{date}][{level}][{id}][{target}] {message}",
                    date = date,
                    level = level,
                    id = conn_id,
                    target = target,
                    message = message
                ))
            } else {
                out.finish(format_args!(
                    "[{date}][{level}][{target}] {message}",
                    date = date,
                    level = level,
                    target = target,
                    message = message
                ))
            }
        })
        .level(level)
        .chain(std::io::stdout())
        .apply()?;

    Ok(())
}

#[derive(Clap)]
#[clap(name = "Comet Tunneling Service")]
struct Opts {
    #[clap(
        short,
        long,
        default_value = "./config.yml",
        about = "Path to configuration file (YAML)"
    )]
    config: String,
    #[clap(
        short,
        long,
        default_value = "info",
        about = "Log level (off, error, warn, info, debug, trace)"
    )]
    level: LevelFilter,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();

    setup_logger(opts.level)?;

    run_bin(&opts.config).await?;
    info!("Service started, press Ctrl-C to stop");

    signal::ctrl_c().await?;
    info!("Ctrl-C received, stopping...");

    Ok(())
}
