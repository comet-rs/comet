use serde::Deserialize;
use std::collections::HashSet;

pub mod sniffer {
  use serde::Deserialize;

  #[derive(Deserialize, Debug, PartialEq, Eq, Hash)]
  #[serde(rename_all(deserialize = "lowercase"))]
  pub enum SniffType {
    Http,
    Tls,
  }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all(deserialize = "snake_case"))]
#[serde(tag = "type")]
pub enum ProcessorItem {
  Sniffer {
    #[serde(default)]
    types: HashSet<sniffer::SniffType>,
  },

  Socks5Server,
  Socks5Client,

  HttpServer,
  HttpClient,

  ShadowsocksServer,
  ShadowsocksClient,

  SsrObfsServer,
  SsrObfsClient,

  VmessServer,
  VmessClient,
}
