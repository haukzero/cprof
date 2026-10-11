use crate::commands::support::{clean, selection::Scope};
use crate::error::Result;
use crate::targets::TargetRepository;

#[cprof_macros::command(no_retry)]
pub fn run(targets: &TargetRepository, force: bool, extra_toml: bool) -> Result<()> {
    clean::run(targets, Scope::Root, force, extra_toml)
}
