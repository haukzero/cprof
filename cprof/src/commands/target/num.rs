use crate::error::Result;
use crate::profile::storage;
use crate::targets::TargetSpec;

#[cprof_macros::command(no_retry)]
pub fn run(target: &TargetSpec) -> Result<()> {
    println!("{}", storage::counts(target)?);
    Ok(())
}
