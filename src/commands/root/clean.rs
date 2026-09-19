use std::fs;

use crate::commands::target;
use crate::config;
use crate::error::Result;
use crate::style;
use crate::targets;

pub fn run(force: bool, extra_toml: bool) -> Result<()> {
    for target in targets::all()?.iter().copied() {
        println!(
            "{}",
            style::heading(&format!("Cleaning target '{}':", target.id))
        );
        target::clean::run(target, force)?;
    }

    if extra_toml {
        let path = config::extra_target_file()?;
        if path.exists() {
            fs::remove_file(&path)?;
            println!("Removed '{}'", path.display());
        }
    }

    Ok(())
}
