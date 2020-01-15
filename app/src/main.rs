use app::run;
use clap::{App, Arg};
use env_logger::Env;
use log::info;
use std::fs;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::from_env(Env::default().default_filter_or("debug")).init();
    let matches = App::new("Ko Ko Da Yo ~")
        .version(env!("CARGO_PKG_VERSION"))
        .about("High performance proxying platform")
        .arg(
            Arg::with_name("config")
                .long("config")
                .value_name("FILE")
                .default_value("./config.json")
                .help("Sets a custom config file")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .get_matches();

    let settings = {
        let path = fs::canonicalize(matches.value_of("config").unwrap())?;
        info!("Using config file {:?}", path);
        settings::load_file(path)?
    };

    run(settings).await?;

    signal::ctrl_c().await?;
    info!("Ctrl-C received, exiting...");

    Ok(())
}
