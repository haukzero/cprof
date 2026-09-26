use std::path::Path;

use crate::commands::support::pack::render_summary;
use crate::error::Result;
use crate::package::{self, pack};
use crate::targets::{TargetRepository, TargetSpec};

pub fn run(targets: &TargetRepository, target: &TargetSpec, save: Option<String>) -> Result<()> {
    let output_path = save.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    let output = Path::new(&output_path);
    let report = pack::create(targets, [target], output)?;
    render_summary(&report, output);
    Ok(())
}
