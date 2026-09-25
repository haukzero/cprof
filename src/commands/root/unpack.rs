use std::path::Path;

use crate::commands::{UnpackPrompt, report_unpack};
use crate::error::Result;
use crate::package::{self, unpack};
use crate::targets::TargetRepository;
use crate::ui::style;

pub fn run(targets: &TargetRepository, path: Option<String>, force: bool) -> Result<()> {
    let path = path.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    let report = unpack::restore(targets, Path::new(&path), None, &mut UnpackPrompt { force })?;
    report_unpack(&report);
    println!();
    style::success(format!(
        "{} unpacked, {} skipped across {} target(s)",
        report.profile_count(),
        report.skipped_count(),
        report.targets.len()
    ));
    Ok(())
}
