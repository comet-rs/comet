use crate::prelude::*;

pub fn register(plumber: &mut Plumber) {
    plumber.register("set_dest", |config, _| {
        let processor: SetDestProcessor = from_value(config)?;
        Ok(Box::new(processor))
    });
}

#[derive(Debug, Deserialize)]
#[serde(tag = "for", rename_all = "lowercase")]
enum SetDestProcessor {
    /// Transport destination for actual (system) connection
    Transport { dest: Option<DestAddr> },
    /// Logical destination for proxying
    Logical { dest: Option<DestAddr> },
}

#[async_trait]
impl Processor for SetDestProcessor {
    async fn prepare(self: Arc<Self>, conn: &mut Connection, _ctx: AppContextRef) -> Result<()> {
        if let Self::Transport { dest } = &*self {
            if let Some(dest) = dest {
                conn.dest_addr = dest.clone();
            } else if let Some(dest) = conn.get_var::<DestAddr>(vars::DEST) {
                conn.dest_addr = dest.clone();
            }
        }

        Ok(())
    }

    async fn process(
        self: Arc<Self>,
        stream: ProxyStream,
        conn: &mut Connection,
        _ctx: AppContextRef,
    ) -> Result<ProxyStream> {
        if let Self::Logical { dest } = &*self {
            if let Some(dest) = dest {
                conn.dest_addr = dest.clone();
            } else if let Some(dest) = conn.get_var::<DestAddr>(vars::DEST) {
                conn.dest_addr = dest.clone();
            }
        }
        Ok(stream)
    }
}
