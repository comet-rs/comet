pub mod inbound;
pub mod input;
pub mod outbound;
pub mod output;
pub mod processor;
pub mod routing;
pub mod transport;
use std::collections::HashMap;

use anyhow::Result;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Settings {
    pub inputs: HashMap<String, input::InputItem>,
    pub outputs: HashMap<String, output::OutputItem>,
}

pub fn load_file<P: AsRef<Path>>(path: P) -> Result<Settings> {
    let mut file = File::open(&path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    let des = toml::from_str(&content)?;
    Ok(des)
}

pub fn load_string(input: &str) -> Result<Settings> {
    let des = toml::from_str(input)?;
    Ok(des)
}
