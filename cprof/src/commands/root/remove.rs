use crate::commands::support::{remove, selection::Scope};
use crate::error::Result;
use crate::targets::TargetRepository;

#[cprof_macros::command(no_retry)]
pub fn run(targets: &TargetRepository, names: Vec<String>, force: bool, ascii: bool) -> Result<()> {
    remove::run(targets, Scope::Root, names, force, ascii)
}
