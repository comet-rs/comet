pub mod inbound;
pub mod outbound;
pub mod routing;
pub mod transport;
use crate::inbound::InboundSettings;
use crate::outbound::OutboundSettings;
use crate::routing::RoutingSettings;
use crate::transport::TransportSettings;

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use log::trace;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Settings {
    #[serde(default)]
    pub inbounds: Vec<InboundSettings>,
    #[serde(default)]
    pub outbounds: Vec<OutboundSettings>,
    #[serde(default)]
    pub transport: TransportSettings,
    #[serde(default)]
    pub routing: RoutingSettings,
}

pub fn load_file<P: AsRef<Path>>(path: P) -> Result<Settings, Box<dyn std::error::Error>> {
    let file = File::open(&path)?;
    let reader = BufReader::new(file);
    let des = serde_json::from_reader(reader)?;
    trace!("Config content: {:#?}", des);
    Ok(des)
}

pub fn load_string(input: &str) -> Result<Settings, Box<dyn std::error::Error>> {
    let des = serde_json::from_str(input)?;
    trace!("Config content: {:#?}", des);
    Ok(des)
}
