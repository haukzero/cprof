use crate::commands::support::{clean, selection::Scope};
use crate::error::Result;
use crate::targets::{TargetRepository, TargetSpec};

#[cprof_macros::command(no_retry)]
pub fn run(targets: &TargetRepository, target: &TargetSpec, force: bool) -> Result<()> {
    clean::run(targets, Scope::Target(&target.id), force, false)
}
