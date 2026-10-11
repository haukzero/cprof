use std::path::{Path, absolute};

use crate::commands::support::unpack::{self, render_preview, render_restore};
use crate::elevate;
use crate::error::Result;
use crate::package;
use crate::targets::TargetRepository;
use crate::ui::style;

#[cprof_macros::command(
    retry = prepared.commit,
    args = ["unpack", "--path", absolute(&path)?.into_os_string()],
    flags = [(force, "--force"), (mirror, "--mirror")],
    context = retry_context,
)]
pub fn run(
    targets: &TargetRepository,
    path: Option<String>,
    force: bool,
    dry_run: bool,
    mirror: bool,
) -> Result<()> {
    let path = path.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    if dry_run {
        let report = unpack::preview(targets, Path::new(&path), None, mirror)?;
        render_preview(&report);
        return Ok(());
    }
    let prepared = unpack::prepare(Path::new(&path), None, force, mirror)?;
    let report = prepared.report();
    let retry_context = prepared.retry_context()?;
    prepared.commit()?;
    if elevate::is_elevated_child() {
        return Ok(());
    }
    render_restore(&report);
    println!();
    style::success(format!(
        "{} unpacked, {} unchanged, {} skipped across {} target(s), {} removed",
        report.profile_count(),
        report.unchanged_count(),
        report.skipped_count(),
        report.targets.len(),
        report.removed_count()
    ));
    Ok(())
}
