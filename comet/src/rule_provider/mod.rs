use std::path::PathBuf;

use crate::{config::Config, prelude::*};

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    #[serde(rename = "v2ray_geoip")]
    V2rayGeoIP,
    #[serde(rename = "v2ray_geosite")]
    V2rayGeoSite,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum ProviderSource {
    Local { path: PathBuf },
    Remote { url: String, interval: u32 },
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProviderConfig {
    format: DataFormat,
    #[serde(flatten)]
    source: ProviderSource,
}

pub struct RuleProviderManager {
    data_dir: PathBuf,
    config: HashMap<SmolStr, ProviderConfig>,
}

impl RuleProviderManager {
    pub fn new(config: &Config) -> Result<Self> {
        let mut providers = config.rule_providers.clone();

        for (_, provider) in providers.iter_mut() {
            if let ProviderSource::Local { ref mut path } = provider.source {
                if !path.is_absolute() {
                    let mut p = config.data_dir.to_path_buf();
                    p.push(&path);
                    *path = p;
                }

                std::fs::metadata(&path)?;
            }
        }

        Ok(Self {
            data_dir: config.data_dir.clone(),
            config: providers,
        })
    }
}
