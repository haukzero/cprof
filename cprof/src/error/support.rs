//! Shared formatting support for domain error messages.

use std::borrow::Cow;
use std::path::Path;

use super::AppError;

/// Simplify Windows paths for messages without changing their stored identity.
pub(super) fn display_path(path: &Path) -> Cow<'_, str> {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};

        let mut components = path.components();
        if let Some(Component::Prefix(prefix)) = components.next() {
            let suffix = components.as_path().display();
            match prefix.kind() {
                Prefix::VerbatimDisk(drive) => {
                    return format!("{}:{suffix}", char::from(drive)).into();
                }
                Prefix::VerbatimUNC(server, share) => {
                    return format!(
                        r"\\{}\{}{suffix}",
                        server.to_string_lossy(),
                        share.to_string_lossy()
                    )
                    .into();
                }
                _ => {}
            }
        }
    }
    path.to_string_lossy()
}

pub(super) fn format_errors(errors: &[AppError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}
