use std::{
    collections::HashSet,
    convert::TryFrom,
    path::{Path, PathBuf},
};

use aho_corasick::AhoCorasickBuilder;
use protobuf::Message;
use regex::Regex;
use tokio::{fs::File, sync::RwLock};

use crate::{
    config::Config,
    prelude::*,
    protos::v2ray::config::{GeoIPList, GeoSite, GeoSiteList},
};

mod rule_set;
use rule_set::RuleSet;

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

type LoadedProvider = HashMap<SmolStr, RuleSet>;

impl ProviderConfig {
    async fn load(&self) -> Result<LoadedProvider> {
        let data: Result<_> = async move {
            Ok(match &self.source {
                ProviderSource::Local { path } => {
                    let mut fd = File::open(path).await?;
                    let mut buf = vec![];
                    fd.read_to_end(&mut buf).await?;
                    self.parse(&buf)?
                }
                ProviderSource::Remote { url, interval: _ } => {
                    let res = reqwest::get(url).await?;
                    let buf = res.bytes().await?;
                    self.parse(&buf)?
                }
            })
        }
        .await;

        if let Err(e) = &data {
            warn!("Failed to load provider: {}", e);
        }

        data
    }

    fn parse(&self, data: &[u8]) -> Result<LoadedProvider> {
        let ret = match self.format {
            DataFormat::V2rayGeoIP => {
                let parsed = GeoIPList::parse_from_bytes(data)?;
                dbg!(&parsed.entry[0]);
                todo!();
            }
            DataFormat::V2rayGeoSite => {
                let parsed = GeoSiteList::parse_from_bytes(data)?;

                parsed
                    .entry
                    .iter()
                    .map(|entry| {
                        let key = entry.country_code.to_ascii_lowercase();
                        let value = RuleSet::try_from(entry)?;
                        Ok((key.into(), value))
                    })
                    .collect::<Result<LoadedProvider>>()?
            }
        };
        Ok(ret)
    }
}

impl TryFrom<&GeoSite> for RuleSet {
    type Error = anyhow::Error;

    fn try_from(value: &GeoSite) -> Result<Self> {
        use crate::protos::v2ray::config::Domain_Type as DomainType;

        let mut full_domains = HashSet::new();
        let mut keywords = vec![];
        let mut domains = vec![];
        let mut regexes = vec![];

        for domain in &value.domain {
            match domain.field_type {
                DomainType::Plain => {
                    keywords.push(SmolStr::from(&domain.value));
                }
                DomainType::Regex => {
                    regexes.push(Regex::new(&domain.value)?);
                }
                DomainType::Domain => {
                    domains.push(rule_set::to_reversed_fqdn(&domain.value));
                }
                DomainType::Full => {
                    full_domains.insert(SmolStr::from(&domain.value));
                }
            }
        }

        let ret = Self::Domain {
            full_domains,
            keywords,
            regexes,
            domains: AhoCorasickBuilder::new()
                .auto_configure(&domains)
                .anchored(true)
                .build(&domains),
        };

        Ok(ret)
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
    state: RwLock<ProviderState>,
}

impl Provider {
    async fn new(tag: &str, config: &ProviderConfig, data_dir: &Path) -> Result<Self> {
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

        let state = match &config.source {
            ProviderSource::Local { .. } => config
                .load()
                .await
                .map(|l| ProviderState::Loaded(l))
                .unwrap_or(ProviderState::Failed),
            ProviderSource::Remote { .. } => ProviderState::NotLoaded,
        };

        Ok(Self {
            path,
            config,
            state: RwLock::new(state),
        })
    }

    fn is_match(self: Arc<Self>, conn: &Connection, sub: &str) -> bool {
        let guard = match self.state.try_read() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        match &*guard {
            ProviderState::NotLoaded => {
                drop(guard);
                self.queue_reload();
                false
            }
            ProviderState::Loaded(rule_sets) => {
                if let Some(rs) = rule_sets.get(sub) {
                    rs.is_match(conn)
                } else {
                    warn!(
                        "Rule provider content tag {} not found, returing unmatched",
                        sub
                    );
                    false
                }
            }
            ProviderState::Failed => false,
        }
    }

    fn queue_reload(self: Arc<Self>) {
        let reload_task = async move {
            let mut guard = self.state.write().await;
            // Make sure that no one has changed the state
            match *guard {
                ProviderState::NotLoaded => {}
                _ => return,
            }

            *guard = self
                .config
                .load()
                .await
                .map(|loaded| ProviderState::Loaded(loaded))
                .unwrap_or(ProviderState::Failed);
        };

        tokio::spawn(reload_task);
    }
}

pub struct RuleProviderManager {
    providers: HashMap<SmolStr, Arc<Provider>>,
}

impl RuleProviderManager {
    pub async fn new(config: &Config) -> Result<Self> {
        let mut providers = HashMap::with_capacity(config.rule_providers.len());
        for (tag, cfg) in config.rule_providers.iter() {
            let provider = Provider::new(tag, cfg, &config.data_dir).await?;
            providers.insert(tag.clone(), Arc::new(provider));
        }
        Ok(Self { providers })
    }

    pub fn is_match(&self, conn: &Connection, tag: &str, sub: &str) -> bool {
        if let Some(provider) = self.providers.get(tag) {
            Arc::clone(provider).is_match(conn, sub)
        } else {
            warn!("Rule provider {} not found, returing unmatched", tag);
            false
        }
    }
}
