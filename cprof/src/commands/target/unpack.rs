use std::path::{Path, absolute};

use crate::commands::support::unpack::{Interactive, Preview, render_preview, render_restore};
use crate::elevate;
use crate::error::Result;
use crate::package::{self, unpack};
use crate::targets::{TargetRepository, TargetSpec};

#[cprof_macros::command(
    retry = plan.commit,
    args = [&target.id, "unpack", "--path", absolute(&path)?.into_os_string()],
    flags = [(force, "--force"), (mirror, "--mirror")],
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
        let mut interaction = Preview;
        let report = unpack::preview(
            targets,
            Path::new(&path),
            Some(&target.id),
            unpack::UnpackMode::from(mirror),
            &mut interaction,
        )?;
        render_preview(&report);
        return Ok(());
    }
    let mut interaction = Interactive::new(force);
    let plan = unpack::prepare(
        targets,
        Path::new(&path),
        Some(&target.id),
        unpack::UnpackMode::from(mirror),
        &mut interaction,
    )?;
    plan.commit()?;
    if !elevate::is_elevated_child() {
        render_restore(&plan.into_report());
    }
    Ok(())
}
