use crate::commands::support::pack::{self, Scope};
use crate::error::Result;
use crate::targets::{TargetRepository, TargetSpec};

#[cprof_macros::command(no_retry)]
pub fn run(
    targets: &TargetRepository,
    target: &TargetSpec,
    save: Option<String>,
    select: Option<Vec<String>>,
    ascii: bool,
) -> Result<()> {
    pack::run(targets, Scope::Target(&target.id), save, select, ascii)
}
