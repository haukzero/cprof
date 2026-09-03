use crate::config;
use crate::error::Result;
use crate::targets::TargetSpec;

pub fn run(target: &TargetSpec) -> Result<()> {
    println!("{}", config::profiles_dir(target)?.display());
    Ok(())
}
