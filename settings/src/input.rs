use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct InputItem {
  port: u16,
  processors: Vec<crate::processor::ProcessorItem>,
}
