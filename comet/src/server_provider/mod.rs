#![allow(clippy::clippy::new_ret_no_self)]

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use serde_with::{serde_as, DurationSeconds};
use tokio::{fs::File, sync::mpsc};

use crate::{config::Config, prelude::*};

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all(deserialize = "snake_case"))]
pub enum ProviderFormat {
    Ssr,
}

#[serde_as]
#[derive(Debug, Deserialize, Clone)]
pub struct ProviderConfig {
    format: ProviderFormat,
    url: url::Url,
    #[serde(default = "default_interval")]
    #[serde_as(as = "DurationSeconds<u64>")]
    interval: Duration,
}

fn default_interval() -> Duration {
    Duration::from_secs(60 * 60)
}

struct Provider {
    path: PathBuf,
    config: ProviderConfig,
}

impl Provider {
    fn new(tag: &str, config: &ProviderConfig, data_dir: &Path) -> Self {
        let config = config.clone();

        let mut path = data_dir.to_path_buf();
        path.push(format!("{}.lst", tag));

        Self { path, config }
    }

    async fn load(&self) -> Result<()> {
        let expired_task = async move {
            let dur = tokio::fs::metadata(&self.path)
                .await?
                .modified()?
                .elapsed()?;
            anyhow::Result::<_, anyhow::Error>::Ok(dur > self.config.interval)
        };
        if expired_task.await.unwrap_or(true) {
            info!("File {:?} has expired, reloading from network", self.path);
            let res = reqwest::get(self.config.url.clone()).await?;
            let buf = res.bytes().await?;
            let mut fd = File::create(&self.path).await?;
            fd.write_all(&buf).await?;
        }

        Ok(())
    }
}

enum ServerListState {}

enum ManagerMessage {}

pub struct ManagerServer {
    tx: mpsc::Sender<ManagerMessage>,
    rx: mpsc::Receiver<ManagerMessage>,
    providers: HashMap<SmolStr, Arc<Provider>>,
}

impl ManagerServer {
    pub fn new(config: &Config) -> Result<ManagerClient> {
        let (tx, rx) = mpsc::channel(1);
        let tx_clone = tx.clone();

        let count = config.server_providers.len();
        let mut providers = HashMap::with_capacity(count);

        for (tag, cfg) in &config.server_providers {
            let provider = Provider::new(tag, cfg, &config.data_dir);
            providers.insert(tag.clone(), Arc::new(provider));
        }

        let this = Self { tx, rx, providers };
        tokio::spawn(this.run());

        Ok(ManagerClient { tx: tx_clone })
    }

    async fn run(self) {
        // let url = "https://api.touhou.center/link/VvyEQXSCboXSqbqR?mu=0";
        // let res = reqwest::get(url).await.unwrap();
        // let buf = res.bytes().await.unwrap();
        // let buf_s = std::str::from_utf8(&buf).unwrap();
        // let parsed = shadowsocks::parse_subscription(buf_s);
        // dbg!(parsed);
    }
}

pub struct ManagerClient {
    tx: mpsc::Sender<ManagerMessage>,
}
