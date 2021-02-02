use tokio::sync::mpsc;

use crate::{config::Config, prelude::*};

enum ManagerMessage {}

pub struct ManagerServer {}

impl ManagerServer {
    pub fn new(config: &Config) -> Result<Self> {

        Ok(Self {})
    }
}
pub struct ManagerClient {
    tx: mpsc::Sender<ManagerMessage>,
}
