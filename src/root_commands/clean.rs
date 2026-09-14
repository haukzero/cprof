use std::fs;

use crate::commands;
use crate::config;
use crate::error::Result;
use crate::style;
use crate::targets::{self, EXTRA_TARGET_FILE};

pub fn run(force: bool, extra_toml: bool) -> Result<()> {
    for target in targets::all()?.iter().copied() {
        println!(
            "{}",
            style::heading(&format!("Cleaning target '{}':", target.id))
        );
        commands::clean::run(target, force)?;
    }

    if extra_toml {
        let path = config::repository_dir()?.join(EXTRA_TARGET_FILE);
        if path.exists() {
            fs::remove_file(&path)?;
            println!("Removed '{}'", path.display());
        }
    }

    Ok(())
}
