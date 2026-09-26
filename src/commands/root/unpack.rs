use std::path::Path;

use crate::commands::support::unpack::{Interactive, Preview, render_preview, render_restore};
use crate::error::Result;
use crate::package::{self, unpack};
use crate::targets::TargetRepository;
use crate::ui::style;

pub fn run(
    targets: &TargetRepository,
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
            None,
            unpack::UnpackMode::from(mirror),
            &mut interaction,
        )?;
        render_preview(&report);
        return Ok(());
    }
    let mut interaction = Interactive::new(force);
    let report = unpack::restore(
        targets,
        Path::new(&path),
        None,
        unpack::UnpackMode::from(mirror),
        &mut interaction,
    )?;
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
