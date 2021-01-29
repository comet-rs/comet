use std::{
    fs::File,
    path::{Path, PathBuf},
};

use protobuf::Message;

use crate::{prelude::*, protos::v2ray::config::GeoSiteList};

#[derive(Debug, Deserialize, Clone)]
pub struct RouterData {
    pub format: DataFormat,
    pub path: PathBuf,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    #[serde(rename = "v2ray_geoip")]
    V2rayGeoIP,
    #[serde(rename = "v2ray_geosite")]
    V2rayGeoSite,
}

impl RouterData {
    pub fn load(&self, base_path: &Path) -> Result<()> {
        let path = if self.path.is_absolute() {
            self.path.clone()
        } else {
            let mut p = base_path.to_path_buf();
            p.push(&self.path);
            p
        };

        let mut file = File::open(path)?;
        match self.format {
            DataFormat::V2rayGeoIP => {}
            DataFormat::V2rayGeoSite => {
                let parsed = GeoSiteList::parse_from_reader(&mut file)?;
                dbg!(&parsed.entry[0]);
            }
        }
        Ok(())
    }
}
