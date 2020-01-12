use common::NetworkType;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "lowercase"))]
pub enum SecurityType {
    None,
    Tls,
}
impl Default for SecurityType {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct StreamSettings {
    #[serde(default)]
    pub network: NetworkType,
    #[serde(default)]
    pub security: SecurityType,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct TransportSettings {}
