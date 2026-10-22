use crate::commands::support::selection::Scope;
use crate::config;
use crate::error::Result;
use crate::filesystem::transaction::PathTransaction;
use crate::profile::{activation, storage};
use crate::targets::{TargetRepository, TargetSpec};
use crate::ui::{prompt, style};

use super::mutation::MutationSession;

pub(crate) fn run(
    targets: &TargetRepository,
    scope: Scope<'_>,
    force: bool,
    extra_toml: bool,
) -> Result<()> {
    if extra_toml {
        // Resolve definitions under their lock before deleting the configuration.
        MutationSession::configuration()?
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
    MutationSession::targets(targets.iter().map(|target| target.id.as_str()))?.run(|| {
        let mut cleaned = Vec::new();
        let mut extra_config = None;
        PathTransaction::prepare(|transaction| {
            for target in targets {
                if matches!(scope, Scope::Root) {
                    println!(
                        "{}",
                        style::heading(&format!("Cleaning target '{}':", target.id))
                    );
                }
                if let Some(count) = stage_clean_target(transaction, &target, force)? {
                    cleaned.push((target.id.clone(), count));
                }
            }
            if extra_toml {
                let path = config::extra_target_file()?;
                if path.exists() {
                    transaction.stage_remove(&path)?;
                    extra_config = Some(path);
                }
            }
            Ok(())
        })?
        .commit()?;

        for (id, count) in cleaned {
            let message = match scope {
                Scope::Root => format!("Cleaned {count} profile(s) from target '{id}'"),
                Scope::Target(_) => format!("Cleaned {count} profile(s)"),
            };
            style::success(message);
        }
        if let Some(path) = extra_config {
            println!("Removed '{}'", path.display());
        }
        Ok(())
    })
}

fn stage_clean_target(
    transaction: &mut PathTransaction,
    target: &TargetSpec,
    force: bool,
) -> Result<Option<usize>> {
    let profiles = storage::names(target)?;
    let active = activation::active_name(target)?;
    if profiles.is_empty() {
        println!("No profiles to clean.");
        return Ok(None);
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
        return Ok(None);
    }
    for name in &profiles {
        activation::LinkPlan::remove_profile(target, name)?.stage(transaction)?;
    }
    let dir = config::profiles_dir(target)?;
    if dir.exists() {
        transaction.stage_remove(&dir)?;
    }
    Ok(Some(profiles.len()))
}
