use crate::prelude::*;

pub fn register(plumber: &mut Plumber) {
    plumber.register("set_dest", |config, _| {
        let processor: SetDestProcessor = from_value(config)?;
        Ok(Box::new(processor))
    });
}

#[derive(Debug, Deserialize)]
struct SetDestProcessor {
    dest: Option<DestAddr>,
}

#[async_trait]
impl Processor for SetDestProcessor {
    async fn prepare(self: Arc<Self>, conn: &mut Connection, _ctx: AppContextRef) -> Result<()> {
        if let Some(dest) = &self.dest {
            conn.dest_addr = dest.clone();
        } else if let Some(dest) = conn.get_var::<DestAddr>(vars::DEST) {
            conn.dest_addr = dest.clone();
        }

        Ok(())
    }
}
