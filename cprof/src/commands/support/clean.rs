use std::fs;

use crate::commands::support::selection::Scope;
use crate::config;
use crate::error::{IoContext, Result};
use crate::profile::{activation, storage};
use crate::targets::{TargetRepository, TargetSpec};
use crate::ui::{prompt, style};

use super::write::WriteSession;

pub(crate) fn run(
    targets: &TargetRepository,
    scope: Scope<'_>,
    force: bool,
    extra_toml: bool,
) -> Result<()> {
    if extra_toml {
        // Resolve definitions under their lock before deleting the configuration.
        WriteSession::configuration()?
            .run(|| clean_profiles(&TargetRepository::load()?, scope, force, extra_toml))
    } else {
        clean_profiles(targets, scope, force, extra_toml)
    }
}

fn clean_profiles(
    targets: &TargetRepository,
    scope: Scope<'_>,
    force: bool,
    extra_toml: bool,
) -> Result<()> {
    let targets = scope.targets(targets)?;
    WriteSession::targets(targets.iter().map(|target| target.id.as_str()))?.run(|| {
        for target in targets {
            if matches!(scope, Scope::Root) {
                println!(
                    "{}",
                    style::heading(&format!("Cleaning target '{}':", target.id))
                );
            }
            clean_target(&target, force)?;
        }

        if extra_toml {
            let path = config::extra_target_file()?;
            if path.exists() {
                fs::remove_file(&path).with_path(&path)?;
                println!("Removed '{}'", path.display());
            }
        }
        Ok(())
    })
}

fn clean_target(target: &TargetSpec, force: bool) -> Result<()> {
    let profiles = storage::names(target)?;
    let active = activation::active_name(target)?;
    if profiles.is_empty() {
        println!("No profiles to clean.");
        return Ok(());
    }
    style::warning("This will remove all profiles and active links:");
    for name in &profiles {
        println!(
            "  {}{}",
            name,
            if active.as_deref() == Some(name.as_str()) {
                format!(" {}", style::active_tag())
            } else {
                String::new()
            }
        );
    }
    if !force && !prompt::confirm("Continue?")? {
        println!("Cancelled.");
        return Ok(());
    }
    for name in &profiles {
        activation::remove_profile_links(target, name)?;
    }
    let dir = config::profiles_dir(target)?;
    if dir.exists() {
        fs::remove_dir_all(&dir).with_path(&dir)?;
    }
    style::success(format!("Cleaned {} profile(s)", profiles.len()));
    Ok(())
}
