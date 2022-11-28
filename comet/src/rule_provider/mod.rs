#![allow(clippy::new_ret_no_self)]

use std::{
    convert::TryFrom,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::anyhow;
use quick_protobuf::{BytesReader, MessageRead};
use tokio::{
    fs::File,
    sync::{mpsc, oneshot},
};

use crate::{
    config::Config,
    prelude::*,
    protos::v2ray::config::{GeoIPList, GeoSiteList},
    router::matching::MatchMode,
};
use serde_with::{serde_as, DurationSeconds};

mod rule_set;
use rule_set::RuleSet;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    #[serde(rename = "v2ray_geoip")]
    V2rayGeoIP,
    #[serde(rename = "v2ray_geosite")]
    V2rayGeoSite,
}

#[serde_as]
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum ProviderSource {
    Local {
        path: PathBuf,
    },
    Remote {
        url: String,
        #[serde(default = "default_interval")]
        #[serde_as(as = "DurationSeconds<u64>")]
        interval: Duration,
    },
}

fn default_interval() -> Duration {
    Duration::from_secs(60 * 60 * 24)
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProviderConfig {
    format: DataFormat,
    #[serde(flatten)]
    source: ProviderSource,
}

type LoadedProvider = HashMap<SmolStr, RuleSet>;

impl ProviderConfig {
    async fn load_from_file(&self, path: &Path, sub: &str) -> Result<RuleSet> {
        let mut fd = File::open(path).await?;
        let mut buf = vec![];
        fd.read_to_end(&mut buf).await?;
        self.parse(&buf, sub)
    }

    fn parse(&self, data: &[u8], sub: &str) -> Result<RuleSet> {
        match self.format {
            DataFormat::V2rayGeoIP => {
                let mut reader = BytesReader::from_bytes(data);
                let parsed = GeoIPList::from_reader(&mut reader, data)?;

                for entry in &parsed.entry {
                    if entry.country_code.eq_ignore_ascii_case(sub) {
                        return RuleSet::try_from(entry);
                    }
                }

                Err(anyhow!("Key not found in rule set"))
            }
            DataFormat::V2rayGeoSite => {
                let mut reader = BytesReader::from_bytes(data);
                let parsed = GeoSiteList::from_reader(&mut reader, data)?;

                for entry in &parsed.entry {
                    if entry.country_code.eq_ignore_ascii_case(sub) {
                        return RuleSet::try_from(entry);
                    }
                }

                Err(anyhow!("Key not found in rule set"))
            }
        }
    }
}

enum ProviderState {
    NotLoaded,
    Loaded(LoadedProvider),
    Failed,
}

pub struct Provider {
    path: PathBuf,
    config: ProviderConfig,
}

impl Provider {
    fn new(tag: &str, config: &ProviderConfig, data_dir: &Path) -> Result<Self> {
        let mut config = config.clone();

        let path = match &mut config.source {
            ProviderSource::Local { path } => {
                let abs_path = if !path.is_absolute() {
                    let mut p = data_dir.to_path_buf();
                    p.push(&path);
                    p
                } else {
                    path.clone()
                };
                std::fs::metadata(&abs_path)?;
                *path = abs_path.clone();
                abs_path
            }
            ProviderSource::Remote { .. } => {
                let mut p = data_dir.to_path_buf();
                p.push(format!("{}.dat", tag));
                p
            }
        };

        Ok(Self { path, config })
    }

    async fn load(&self, sub: &str, _reload: bool) -> Result<RuleSet> {
        match &self.config.source {
            ProviderSource::Local { .. } => {}
            ProviderSource::Remote { url, interval } => {
                let expired_task = async move {
                    let dur = tokio::fs::metadata(&self.path)
                        .await?
                        .modified()?
                        .elapsed()?;
                    anyhow::Result::<_, anyhow::Error>::Ok(dur > *interval)
                };
                if expired_task.await.unwrap_or(true) {
                    info!("File {:?} has expired, reloading from network", self.path);
                    let res = reqwest::get(url).await?;
                    let buf = res.bytes().await?;
                    let mut fd = File::create(&self.path).await?;
                    fd.write_all(&buf).await?;
                }
            }
        }
        self.config.load_from_file(&self.path, sub).await
    }
}

enum ManagerMessage {
    Match {
        tx: oneshot::Sender<bool>,
        tag: SmolStr,
        sub: SmolStr,
        dest: DestAddr,
        mode: MatchMode,
    },
    Load {
        tag: SmolStr,
        sub: SmolStr,
    },
    Insert {
        tag: SmolStr,
        sub: SmolStr,
        rule_set: Option<RuleSet>,
    },
}

enum RuleSetState {
    Loading, // Loading or failed
    Loaded(RuleSet),
}

type LoadedItem = HashMap<SmolStr, RuleSetState>;

pub struct RuleProviderServer {
    tx: mpsc::Sender<ManagerMessage>,
    rx: mpsc::Receiver<ManagerMessage>,
    loaded: HashMap<SmolStr, LoadedItem>,
    providers: HashMap<SmolStr, Arc<Provider>>,
}

impl RuleProviderServer {
    pub fn new(config: &Config) -> Result<RuleProviderClient> {
        let (tx, rx) = mpsc::channel(1);
        let tx_clone = tx.clone();

        let count = config.rule_providers.len();
        let mut providers = HashMap::with_capacity(count);
        let mut loaded = HashMap::with_capacity(count);

        for (tag, cfg) in config.rule_providers.iter() {
            let provider = Provider::new(tag, cfg, &config.data_dir)?;
            providers.insert(tag.clone(), Arc::new(provider));
            loaded.insert(tag.clone(), HashMap::new());
        }

        let this = Self {
            tx,
            rx,
            loaded,
            providers,
        };
        tokio::spawn(this.run());

        Ok(RuleProviderClient { tx: tx_clone })
    }

    async fn run(mut self) {
        while let Some(msg) = self.rx.recv().await {
            self.handle_message(msg).await;
        }
    }

    async fn handle_message(&mut self, msg: ManagerMessage) {
        match msg {
            ManagerMessage::Match {
                tx: rx,
                tag,
                sub,
                dest,
                mode,
            } => {
                let result = match self.loaded.get(&tag) {
                    Some(inner) => match inner.get(&sub) {
                        Some(RuleSetState::Loaded(rule_set)) => rule_set.is_match(dest, mode),
                        Some(RuleSetState::Loading) => false, // Failed/Loading
                        None => {
                            // Load new
                            let _ = self.tx.send(ManagerMessage::Load { tag, sub }).await;
                            false
                        }
                    },
                    None => {
                        warn!("Rule provider {} not found, returning unmatched", tag);
                        false
                    }
                };
                let _ = rx.send(result);
            }
            ManagerMessage::Load { tag, sub } => {
                let self_tx = self.tx.clone();
                let provider = self.providers.get(&tag).unwrap().clone();
                self.loaded
                    .get_mut(&tag)
                    .unwrap()
                    .insert(sub.clone(), RuleSetState::Loading);

                tokio::spawn(async move {
                    let res = match provider.load(&sub, false).await {
                        Ok(r) => {
                            info!("Loaded rule set {}:{}", tag, sub);
                            Some(r)
                        }
                        Err(e) => {
                            warn!("Failed to load {}:{}: {}", tag, sub, e);
                            None
                        }
                    };
                    let _ = self_tx
                        .send(ManagerMessage::Insert {
                            tag,
                            sub,
                            rule_set: res,
                        })
                        .await;
                });
            }
            ManagerMessage::Insert { tag, sub, rule_set } => {
                let inner = self.loaded.get_mut(&tag).unwrap();
                inner.insert(
                    sub,
                    rule_set
                        .map(RuleSetState::Loaded)
                        .unwrap_or(RuleSetState::Loading),
                );
            }
        }
    }
}

pub struct RuleProviderClient {
    tx: mpsc::Sender<ManagerMessage>,
}

impl RuleProviderClient {
    pub async fn is_match(&self, tag: &str, sub: &str, dest: &DestAddr, mode: MatchMode) -> bool {
        let dest = dest.clone();
        let (tx, rx) = oneshot::channel();
        let msg = ManagerMessage::Match {
            tx,
            tag: tag.into(),
            sub: sub.into(),
            dest,
            mode,
        };
        let _ = self.tx.send(msg).await;
        match rx.await {
            Ok(res) => res,
            Err(_) => {
                warn!("RuleProviderServer has failed (channel closed w/o result)");
                false
            }
        }
    }
}
