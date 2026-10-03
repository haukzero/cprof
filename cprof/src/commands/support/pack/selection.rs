//! Resolve CLI selectors and the interactive tree into the same package input.

use crate::commands::support::selection::{self, Scope};
use crate::error::{PackageError, Result};
use crate::profile::SelectedTarget;
use crate::targets::TargetRepository;
use crate::ui::tree::TreeStyle;

pub(crate) fn resolve(
    targets: &TargetRepository,
    scope: Scope<'_>,
    selectors: Option<Vec<String>>,
    style: TreeStyle,
) -> Result<Vec<SelectedTarget>> {
    match selectors {
        None => selection::available(targets, scope),
        Some(selectors) if selectors.is_empty() => {
            let branches = selection::available(targets, scope)?
                .into_iter()
                .map(|selection| {
                    selection::branch(
                        selection.target,
                        selection
                            .profiles
                            .into_iter()
                            .map(|name| (name.clone(), name)),
                    )
                })
                .collect();
            selection::select(
                "Select profiles to pack",
                branches,
                style,
                PackageError::Invalid("No profiles to pack".to_string()).into(),
            )
        }
        Some(selectors) => selection::resolve_explicit(targets, scope, selectors, |_, name| {
            Ok(vec![name.to_string()])
        }),
    }
}
