//! Shared target/profile selection; commands supply matching rules and display labels.

use std::sync::Arc;

use indexmap::IndexMap;

use crate::error::{AppError, Result};
use crate::profile::{SelectedTarget, storage};
use crate::targets::{TargetRepository, TargetSpec};
use crate::ui::tree::{self, Item, TreeStyle};

#[derive(Clone, Copy)]
pub(crate) enum Scope<'a> {
    Root,
    Target(&'a str),
}

impl Scope<'_> {
    pub(crate) fn targets(self, targets: &TargetRepository) -> Result<Vec<Arc<TargetSpec>>> {
        match self {
            Self::Root => Ok(targets.all().to_vec()),
            Self::Target(id) => Ok(vec![targets.get(id)?]),
        }
    }

    pub(crate) fn label(self, target: &TargetSpec, name: &str) -> String {
        match self {
            Self::Root => format!("{}/{name}", target.id),
            Self::Target(_) => name.to_string(),
        }
    }
}

pub(crate) fn available(
    targets: &TargetRepository,
    scope: Scope<'_>,
) -> Result<Vec<SelectedTarget>> {
    scope
        .targets(targets)?
        .into_iter()
        .map(|target| {
            Ok(SelectedTarget {
                profiles: storage::names(&target)?.into_iter().collect(),
                target,
            })
        })
        .collect()
}

/// Commands resolve exact names or patterns; validation and merging stay shared.
pub(crate) fn resolve_explicit(
    targets: &TargetRepository,
    scope: Scope<'_>,
    selectors: Vec<String>,
    resolve_names: impl Fn(&TargetSpec, &str) -> Result<Vec<String>>,
) -> Result<Vec<SelectedTarget>> {
    let mut selected = IndexMap::new();
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
            Some(profile) => resolve_names(&target, profile)?,
            None => storage::names(&target)?,
        };
        for name in &names {
            storage::require_exists(&target, name)?;
        }
        extend(&mut selected, target, names);
    }
    Ok(selected.into_values().collect())
}

/// Build a target branch from (profile name, display label) pairs.
pub(crate) fn branch(
    target: Arc<TargetSpec>,
    profiles: impl IntoIterator<Item = (String, String)>,
) -> Item<(Arc<TargetSpec>, String)> {
    let children: Vec<_> = profiles
        .into_iter()
        .map(|(name, label)| Item::Leaf(label, (Arc::clone(&target), name)))
        .collect();
    let label = if children.is_empty() {
        format!("{} (no profiles)", target.id)
    } else {
        target.id.clone()
    };
    Item::Branch(label, children)
}

pub(crate) fn select(
    prompt: &str,
    branches: Vec<Item<(Arc<TargetSpec>, String)>>,
    style: TreeStyle,
    empty_error: AppError,
) -> Result<Vec<SelectedTarget>> {
    let root = Item::Branch("All profiles".into(), branches);
    if root.is_empty() {
        return Err(empty_error);
    }
    let mut selected = IndexMap::new();
    for (target, name) in tree::select(prompt, root, style)? {
        extend(&mut selected, target, [name]);
    }
    Ok(selected.into_values().collect())
}

fn extend(
    selected: &mut IndexMap<String, SelectedTarget>,
    target: Arc<TargetSpec>,
    names: impl IntoIterator<Item = String>,
) {
    selected
        .entry(target.id.clone())
        .or_insert_with(|| SelectedTarget {
            target,
            profiles: Default::default(),
        })
        .profiles
        .extend(names);
}
