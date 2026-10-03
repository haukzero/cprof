//! Resolve CLI selectors and the interactive tree into the same package input.

use indexmap::IndexMap;

use crate::error::{PackageError, Result};
use crate::package::pack::SelectedTarget;
use crate::profile::storage;
use crate::targets::TargetRepository;
use crate::ui::tree::{self, Item, TreeStyle};

#[derive(Clone, Copy)]
pub(crate) enum Scope<'a> {
    Root,
    Target(&'a str),
}

pub(crate) fn resolve(
    targets: &TargetRepository,
    scope: Scope<'_>,
    selectors: Option<Vec<String>>,
    style: TreeStyle,
) -> Result<Vec<SelectedTarget>> {
    match selectors {
        None => available(targets, scope),
        Some(selectors) if selectors.is_empty() => {
            select_interactively(available(targets, scope)?, style)
        }
        Some(selectors) => resolve_explicit(targets, scope, selectors),
    }
}

fn available(targets: &TargetRepository, scope: Scope<'_>) -> Result<Vec<SelectedTarget>> {
    let targets = match scope {
        Scope::Root => targets.all().to_vec(),
        Scope::Target(id) => vec![targets.get(id)?],
    };
    targets
        .into_iter()
        .map(|target| {
            Ok(SelectedTarget {
                profiles: storage::names(&target)?.into_iter().collect(),
                target,
            })
        })
        .collect()
}

fn resolve_explicit(
    targets: &TargetRepository,
    scope: Scope<'_>,
    selectors: Vec<String>,
) -> Result<Vec<SelectedTarget>> {
    let mut selected = IndexMap::<String, SelectedTarget>::new();
    for selector in selectors {
        let (id, profile) = match scope {
            Scope::Root => match selector.split_once('/') {
                Some((id, profile)) => (id, Some(profile)),
                None => (selector.as_str(), None),
            },
            Scope::Target(id) => (id, Some(selector.as_str())),
        };
        let target = targets.get(id)?;
        let names = match profile {
            Some(name) => {
                storage::require_exists(&target, name)?;
                vec![name.to_string()]
            }
            None => storage::names(&target)?,
        };
        selected
            .entry(id.to_string())
            .or_insert_with(|| SelectedTarget {
                target,
                profiles: Default::default(),
            })
            .profiles
            .extend(names);
    }
    Ok(selected.into_values().collect())
}

fn select_interactively(
    mut available: Vec<SelectedTarget>,
    style: TreeStyle,
) -> Result<Vec<SelectedTarget>> {
    if available.iter().all(|target| target.profiles.is_empty()) {
        return Err(PackageError::Invalid("No profiles to pack".to_string()).into());
    }
    let items = available
        .iter()
        .map(|selection| {
            let label = if selection.profiles.is_empty() {
                format!("{} (no profiles)", selection.target.id)
            } else {
                selection.target.id.clone()
            };
            Item::Branch(
                label,
                selection.profiles.iter().cloned().map(Item::Leaf).collect(),
            )
        })
        .collect();
    let selected = tree::select(
        "Select profiles to pack",
        Item::Branch("All profiles".into(), items),
        style,
    )?;
    // Leaf indices follow target/profile display order; branches never become payloads.
    let mut selected = selected.into_iter().peekable();
    let mut index = 0;
    for target in &mut available {
        target.profiles.retain(|_| {
            let keep = selected.peek() == Some(&index);
            if keep {
                selected.next();
            }
            index += 1;
            keep
        });
    }
    available.retain(|target| !target.profiles.is_empty());
    Ok(available)
}
