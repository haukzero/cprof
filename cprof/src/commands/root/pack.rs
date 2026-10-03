use crate::commands::support::pack::{self, Scope};
use crate::error::Result;
use crate::targets::TargetRepository;

#[cprof_macros::command(no_retry)]
pub fn run(
    targets: &TargetRepository,
    save: Option<String>,
    select: Option<Vec<String>>,
    ascii: bool,
) -> Result<()> {
    pack::run(targets, Scope::Root, save, select, ascii)
}
