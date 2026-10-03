use crate::commands::support::{remove, selection::Scope};
use crate::error::Result;
use crate::targets::{TargetRepository, TargetSpec};

#[cprof_macros::command(no_retry)]
pub fn run(
    targets: &TargetRepository,
    target: &TargetSpec,
    names: Vec<String>,
    force: bool,
    ascii: bool,
) -> Result<()> {
    remove::run(targets, Scope::Target(&target.id), names, force, ascii)
}
