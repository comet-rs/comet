use anyhow::Result;
use settings::load_file;

fn main() -> Result<()>{
  let config = load_file("./config.toml")?;
  println!("{:#?}", config);
  Ok(())
}