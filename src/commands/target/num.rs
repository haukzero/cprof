use crate::error::Result;
use crate::profile::storage;
use crate::targets::TargetSpec;

pub fn run(target: &TargetSpec) -> Result<()> {
    println!("{}", storage::counts(target)?);
    Ok(())
}
