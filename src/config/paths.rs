use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::result;

use crate::error::{AppError, IoContext, Result};

/// Validate a value that is used as one directory component below cprof's
/// profile root.
pub(crate) fn validate_target_id(value: &str) -> Result<()> {
    validate_safe_component(value).map_err(|()| AppError::InvalidTargetId(value.to_string()))
}

/// Validate a resource's stored filename. Resource files always live directly
/// in a profile directory; accepting a path here would cross that boundary.
pub(crate) fn validate_filename(value: &str) -> Result<()> {
    if is_windows_reserved_component(value) {
        return Err(AppError::InvalidFilename(value.to_string()));
    }
    validate_safe_component(value).map_err(|()| AppError::InvalidFilename(value.to_string()))
}

fn is_windows_reserved_component(value: &str) -> bool {
    let stem = value
        .split('.')
        .next()
        .unwrap_or(value)
        .trim_end_matches(' ');
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || upper
            .strip_prefix("COM")
            .or_else(|| upper.strip_prefix("LPT"))
            .is_some_and(|number| {
                matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
}

pub(crate) fn validate_safe_component(value: &str) -> result::Result<(), ()> {
    if value.is_empty()
        || matches!(value, "." | "..")
        || value.contains(['/', '\\'])
        || value.chars().any(char::is_control)
        || Path::new(value).is_absolute()
        || has_windows_prefix(value)
    {
        return Err(());
    }

    let mut components = Path::new(value).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(component)), None) if component == OsStr::new(value) => Ok(()),
        _ => Err(()),
    }
}

/// Normalize and validate an active path relative to HOME. Both slash forms
/// are interpreted as separators so a package cannot be safe on one platform
/// and traverse on another.
pub(crate) fn relative_active_path(value: &str) -> Result<PathBuf> {
    if value.is_empty()
        || value.chars().any(char::is_control)
        || value.starts_with(['/', '\\'])
        || Path::new(value).is_absolute()
        || has_windows_prefix(value)
    {
        return Err(AppError::InvalidActivePath(value.to_string()));
    }

    let mut normalized = PathBuf::new();
    for component in value.split(['/', '\\']) {
        match component {
            "" | "." => {}
            ".." => return Err(AppError::InvalidActivePath(value.to_string())),
            component
                if has_windows_prefix(component) || is_windows_reserved_component(component) =>
            {
                return Err(AppError::InvalidActivePath(value.to_string()));
            }
            component => normalized.push(component),
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(AppError::InvalidActivePath(value.to_string()));
    }
    Ok(normalized)
}

/// Normalize and validate an explicitly absolute active path.
pub(crate) fn absolute_active_path(value: &str) -> Result<PathBuf> {
    if value.is_empty()
        || value.chars().any(char::is_control)
        || (!cfg!(windows) && value.contains('\\'))
    {
        return Err(AppError::InvalidActivePath(value.to_string()));
    }
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err(AppError::InvalidActivePath(value.to_string()));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(AppError::InvalidActivePath(value.to_string()));
            }
            Component::Normal(component) => {
                if is_windows_reserved_component(&component.to_string_lossy()) {
                    return Err(AppError::InvalidActivePath(value.to_string()));
                }
                normalized.push(component);
            }
        }
    }
    if normalized.parent().is_none() || normalized.file_name().is_none() {
        return Err(AppError::InvalidActivePath(value.to_string()));
    }
    Ok(normalized)
}

/// Join a validated relative path to an allowed root and verify both the
/// lexical result and the resolution of all existing path components. This
/// catches a parent-directory symlink that redirects the result outside root.
pub(crate) fn join_under(root: &Path, relative: &Path) -> Result<PathBuf> {
    let candidate = join_relative(root, relative)?;
    ensure_resolves_under(root, &candidate)?;
    Ok(candidate)
}

fn join_relative(root: &Path, relative: &Path) -> Result<PathBuf> {
    if !root.is_absolute() {
        return Err(AppError::UnsafePath(root.display().to_string()));
    }
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::Prefix(_) | Component::RootDir | Component::ParentDir
            )
        })
    {
        return Err(AppError::UnsafePath(relative.display().to_string()));
    }

    let root = lexical_normalize(root)?;
    let candidate = lexical_normalize(&root.join(relative))?;
    if !candidate.starts_with(&root) || candidate == root {
        return Err(AppError::UnsafePath(candidate.display().to_string()));
    }
    Ok(candidate)
}

/// Resolve a path below a storage root and reject an existing symlink at the
/// final entry. Profile directories and files are managed objects, not links;
/// allowing a leaf link would let one profile alias another profile's file
/// even when both targets remain lexically inside the allowed root.
pub(crate) fn join_storage_under(root: &Path, relative: &Path) -> Result<PathBuf> {
    let path = join_under(root, relative)?;
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(AppError::UnsafePath(path.display().to_string()));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(AppError::io(&path, error)),
    }
    Ok(path)
}

/// Like `join_under`, but permits the leaf itself to be a symlink. Active
/// resources are links by design; only their parent directories must remain
/// below HOME.
pub(crate) fn join_active_under(root: &Path, relative: &Path) -> Result<PathBuf> {
    let candidate = join_relative(root, relative)?;
    ensure_resolves_under(root, candidate.parent().expect("path below root"))?;
    ensure_active_entry(&candidate)?;
    Ok(candidate)
}

fn ensure_active_entry(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            return Err(AppError::UnsafePath(path.display().to_string()));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(AppError::io(path, error)),
    }
    Ok(())
}

/// Verify an absolute active path. There is intentionally no containment root
/// for this opt-in form, but resolving its existing parent now catches broken
/// or cyclic parent symlinks before any mutation is attempted.
pub(crate) fn checked_absolute_active(value: &str) -> Result<PathBuf> {
    let path = absolute_active_path(value)?;
    let parent = path
        .parent()
        .ok_or_else(|| AppError::InvalidActivePath(value.to_string()))?;
    resolve_existing(parent)?;
    ensure_active_entry(&path)?;
    Ok(path)
}

/// Resolve either spelling of an active path, enforcing the HOME boundary for
/// relative paths and rejecting conflicting dual spellings.
pub(crate) fn resolve_active_path(
    home: &Path,
    relative: Option<&str>,
    absolute: Option<&str>,
) -> Result<PathBuf> {
    let relative = relative
        .map(relative_active_path)
        .transpose()?
        .map(|path| join_active_under(home, &path))
        .transpose()?;
    let absolute = absolute.map(checked_absolute_active).transpose()?;
    match (relative, absolute) {
        (Some(relative), None) => Ok(relative),
        (None, Some(absolute)) => Ok(absolute),
        (Some(relative), Some(absolute)) => {
            if path_identity(&relative)? != path_identity(&absolute)? {
                return Err(AppError::InvalidActivePath(
                    "conflicting active_path and absolute_active_path".to_string(),
                ));
            }
            Ok(absolute)
        }
        (None, None) => Err(AppError::InvalidActivePath(
            "must define active_path or absolute_active_path".to_string(),
        )),
    }
}

/// Return a stable identity for duplicate/conflict checks. Existing parent
/// symlinks are resolved and a non-existent filename is appended unchanged.
pub(crate) fn path_identity(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::UnsafePath(path.display().to_string()))?;
    let filename = path
        .file_name()
        .ok_or_else(|| AppError::UnsafePath(path.display().to_string()))?;
    Ok(resolve_existing(parent)?.join(filename))
}

fn ensure_resolves_under(root: &Path, candidate: &Path) -> Result<()> {
    let resolved_root = resolve_existing(root)?;
    let resolved = resolve_existing(candidate)?;
    if !resolved.starts_with(&resolved_root) {
        return Err(AppError::UnsafePath(candidate.display().to_string()));
    }
    Ok(())
}

/// Canonicalize the longest existing prefix, then append the missing suffix.
/// Unlike a simple `canonicalize(parent)`, this also spots symlinks above a
/// path whose immediate parent has not been created yet.
pub(crate) fn resolve_existing(path: &Path) -> Result<PathBuf> {
    // Resolve existing symlinks before interpreting `..`: lexically removing
    // it first changes the meaning of paths such as `alias/../profile`.
    let mut current = path.to_path_buf();
    let mut missing: Vec<OsString> = Vec::new();
    loop {
        match fs::symlink_metadata(&current) {
            Ok(_) => {
                let mut resolved = fs::canonicalize(&current).with_path(&current)?;
                for component in missing.iter().rev() {
                    resolved.push(component);
                }
                return lexical_normalize(&resolved);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let component = current
                    .file_name()
                    .ok_or_else(|| AppError::UnsafePath(path.display().to_string()))?;
                missing.push(component.to_os_string());
                current = current
                    .parent()
                    .ok_or_else(|| AppError::UnsafePath(path.display().to_string()))?
                    .to_path_buf();
            }
            Err(error) => return Err(AppError::io(&current, error)),
        }
    }
}

fn lexical_normalize(path: &Path) -> Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(AppError::UnsafePath(path.display().to_string()));
                }
            }
            Component::Normal(component) => normalized.push(component),
        }
    }
    Ok(normalized)
}

fn has_windows_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
        || value.starts_with("\\\\")
        || value.starts_with("//")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_identity_normalizes_parents_without_requiring_the_leaf() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("active")).unwrap();
        fs::create_dir(directory.path().join("profiles")).unwrap();
        let relative_target = directory
            .path()
            .join("active/../profiles/missing/config.toml");
        let expected = fs::canonicalize(directory.path())
            .unwrap()
            .join("profiles/missing/config.toml");
        assert_eq!(path_identity(&relative_target).unwrap(), expected);
    }

    #[cfg(windows)]
    #[test]
    fn entry_identity_matches_normal_and_verbatim_windows_paths() {
        let directory = tempfile::tempdir().unwrap();
        let canonical = fs::canonicalize(directory.path()).unwrap();
        assert_eq!(
            path_identity(&directory.path().join("config.toml")).unwrap(),
            path_identity(&canonical.join("config.toml")).unwrap()
        );
    }

    #[test]
    fn single_components_reject_traversal_and_platform_separators() {
        for value in ["", ".", "..", "a/b", "a\\b", "/tmp", "C:\\tmp", "x\0y"] {
            assert!(validate_target_id(value).is_err(), "{value:?}");
            assert!(validate_filename(value).is_err(), "{value:?}");
        }
        for value in ["demo", "settings.json", "配置"] {
            assert!(validate_target_id(value).is_ok(), "{value:?}");
            assert!(validate_filename(value).is_ok(), "{value:?}");
        }
    }

    #[test]
    fn filenames_reject_windows_reserved_names_on_every_platform() {
        for value in [
            "CON", "con.txt", "PRN.json", "AUX", "NUL", "COM1.log", "LPT9",
        ] {
            assert!(validate_filename(value).is_err(), "{value:?}");
        }
        for value in ["console", "COM0", "COM10", "LPT10.json"] {
            assert!(validate_filename(value).is_ok(), "{value:?}");
        }
    }

    #[test]
    fn relative_paths_normalize_but_never_traverse() {
        assert_eq!(
            relative_active_path("./.config//demo\\settings.json").unwrap(),
            PathBuf::from(".config/demo/settings.json")
        );
        for value in [
            "",
            ".",
            "..",
            "../x",
            "a/../x",
            "/tmp/x",
            "\\tmp\\x",
            "C:\\x",
            ".config/CON/settings.json",
        ] {
            assert!(relative_active_path(value).is_err(), "{value:?}");
        }
    }
}
