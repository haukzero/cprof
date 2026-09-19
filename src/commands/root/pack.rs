use std::path::Path;

use crate::commands::target;
use crate::error::{AppError, Result};
use crate::package;
use crate::targets::TargetRepository;

pub fn run(targets: &TargetRepository, save: Option<String>) -> Result<()> {
    let output_path = save.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    let output = Path::new(&output_path);
    let packages = targets
        .all()
        .iter()
        .map(|target| target::pack::collect_target(target))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if packages.is_empty() {
        return Err(AppError::InvalidPackage("No profiles to pack".to_string()));
    }
    target::pack::write_package(targets, output, &packages)
}
