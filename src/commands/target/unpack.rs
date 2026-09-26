use std::path::Path;

use crate::commands::support::unpack::{Interactive, Preview, render_preview, render_restore};
use crate::error::Result;
use crate::package::{self, unpack};
use crate::targets::{TargetRepository, TargetSpec};

pub fn run(
    targets: &TargetRepository,
    target: &TargetSpec,
    path: Option<String>,
    force: bool,
    dry_run: bool,
) -> Result<()> {
    let path = path.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    if dry_run {
        let mut interaction = Preview;
        let report = unpack::preview(
            targets,
            Path::new(&path),
            Some(&target.id),
            &mut interaction,
        )?;
        render_preview(&report);
        return Ok(());
    }
    let mut interaction = Interactive::new(force);
    let report = unpack::restore(
        targets,
        Path::new(&path),
        Some(&target.id),
        &mut interaction,
    )?;
    render_restore(&report);
    Ok(())
}
