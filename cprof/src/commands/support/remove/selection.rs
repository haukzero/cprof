//! Resolve removal selectors and tree choices before changing any profiles.

use crate::commands::support::selection::{self, Scope};
use crate::error::{ProfileError, Result};
use crate::profile::{SelectedTarget, activation, storage};
use crate::targets::TargetRepository;
use crate::ui::tree::TreeStyle;

pub(super) fn resolve(
    targets: &TargetRepository,
    scope: Scope<'_>,
    selectors: Vec<String>,
    style: TreeStyle,
) -> Result<Vec<SelectedTarget>> {
    if selectors.is_empty() {
        return interact_select(targets, scope, style);
    }

    selection::resolve_explicit(targets, scope, selectors, |target, pattern| {
        storage::resolve_names(target, &[pattern.to_string()])
    })
}

fn interact_select(
    targets: &TargetRepository,
    scope: Scope<'_>,
    style: TreeStyle,
) -> Result<Vec<SelectedTarget>> {
    let mut branches = Vec::new();
    for target in scope.targets(targets)? {
        let profiles = storage::list(&target)?;
        let active = activation::active_name_from_profiles(&target, &profiles)?;
        branches.push(selection::branch(
            target,
            profiles.into_iter().map(|profile| {
                let status = if active.as_deref() == Some(profile.name.as_str()) {
                    " (active)"
                } else if !profile.complete {
                    " (incomplete)"
                } else {
                    ""
                };
                let label = format!("{}{status}", profile.name);
                (profile.name, label)
            }),
        ));
    }
    selection::select(
        "Select profiles to remove",
        branches,
        style,
        ProfileError::NotFound("(no profiles exist)".to_string()).into(),
    )
}
