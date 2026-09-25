use std::path::Path;

use crate::commands::{UnpackPrompt, report_unpack};
use crate::error::Result;
use crate::package::{self, unpack};
use crate::targets::{TargetRepository, TargetSpec};

pub fn run(
    targets: &TargetRepository,
    target: &TargetSpec,
    path: Option<String>,
    force: bool,
) -> Result<()> {
    let path = path.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    let report = unpack::restore(
        targets,
        Path::new(&path),
        Some(&target.id),
        &mut UnpackPrompt { force },
    )?;
    report_unpack(&report);
    Ok(())
}
