use std::fs;

use crate::commands::target;
use crate::config;
use crate::error::{IoContext, Result};
use crate::style;
use crate::targets::TargetRepository;

pub fn run(targets: &TargetRepository, force: bool, extra_toml: bool) -> Result<()> {
    for target in targets.all() {
        println!(
            "{}",
            style::heading(&format!("Cleaning target '{}':", target.id))
        );
        target::clean::run(target, force)?;
    }

    if extra_toml {
        let path = config::extra_target_file()?;
        if path.exists() {
            fs::remove_file(&path).with_path(&path)?;
            println!("Removed '{}'", path.display());
        }
    }

    Ok(())
}
