use std::path::{Path, absolute};

use crate::commands::support::unpack::{self, render_preview, render_restore};
use crate::elevate;
use crate::error::Result;
use crate::package;
use crate::targets::{TargetRepository, TargetSpec};

#[cprof_macros::command(
    retry = prepared.commit,
    args = [&target.id, "unpack", "--path", absolute(&path)?.into_os_string()],
    flags = [(force, "--force"), (mirror, "--mirror")],
    context = retry_context,
)]
pub fn run(
    targets: &TargetRepository,
    target: &TargetSpec,
    path: Option<String>,
    force: bool,
    dry_run: bool,
    mirror: bool,
) -> Result<()> {
    let path = path.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    if dry_run {
        let report = unpack::preview(targets, Path::new(&path), Some(&target.id), mirror)?;
        render_preview(&report);
        return Ok(());
    }
    let prepared = unpack::prepare(Path::new(&path), Some(&target.id), force, mirror)?;
    let report = prepared.report();
    let retry_context = prepared.retry_context()?;
    prepared.commit()?;
    if !elevate::is_elevated_child() {
        render_restore(&report);
    }
    Ok(())
}
