use crate::error::Result;
use crate::profile;
use crate::targets::TargetSpec;

pub fn run(target: &TargetSpec) -> Result<()> {
    println!("{}", profile::names(target)?.len());
    Ok(())
}
