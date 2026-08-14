use crate::config;
use crate::error::Result;

pub fn run() -> Result<()> {
    let dir = config::profiles_dir()?;
    println!("{}", dir.display());
    Ok(())
}
