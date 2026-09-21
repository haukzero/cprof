use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use crate::commands::target;
use crate::error::{AppError, Result};
use crate::package;
use crate::targets::{TargetRepository, TargetSpec};
use crate::ui::prompt;

pub fn run(
    targets: &TargetRepository,
    save: Option<String>,
    select: Option<Vec<String>>,
) -> Result<()> {
    let output_path = save.unwrap_or_else(|| package::DEFAULT_FILE_NAME.to_string());
    let output = Path::new(&output_path);
    let selected_targets = selected_targets(targets, select)?;
    if selected_targets.is_empty() {
        return Err(AppError::NoTargetsSelected);
    }
    let packages = selected_targets
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

fn selected_targets(
    targets: &TargetRepository,
    select: Option<Vec<String>>,
) -> Result<Vec<Arc<TargetSpec>>> {
    match select {
        None => Ok(targets.all().to_vec()),
        Some(target_ids) if target_ids.is_empty() => prompt::select_targets(targets.all()),
        Some(target_ids) => {
            let mut seen = HashSet::new();
            target_ids
                .into_iter()
                .filter(|target_id| seen.insert(target_id.clone()))
                .map(|target_id| targets.get(&target_id))
                .collect()
        }
    }
}
