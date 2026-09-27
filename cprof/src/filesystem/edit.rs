use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

pub(crate) fn edit_draft_path(destination: &Path) -> PathBuf {
    let mut name = OsString::from(".");
    name.push(
        destination
            .file_stem()
            .unwrap_or_else(|| destination.file_name().unwrap_or(destination.as_os_str())),
    );
    name.push(".cprof-edit");
    if let Some(extension) = destination.extension() {
        name.push(".");
        name.push(extension);
    }
    destination.with_file_name(name)
}

pub(crate) fn has_edit_draft(destination: &Path) -> bool {
    edit_draft_path(destination).is_file()
}

/// Create the stable draft used by an edit transaction, or reuse an existing
/// draft left by an interrupted editor session.
pub(crate) fn create_edit_file(destination: &Path, content: &[u8]) -> io::Result<PathBuf> {
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let path = edit_draft_path(destination);
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            if let Err(error) = super::write_and_flush(&mut file, content) {
                let _ = fs::remove_file(&path);
                return Err(error);
            }
            if let Ok(metadata) = fs::metadata(destination)
                && let Err(error) = fs::set_permissions(&path, metadata.permissions())
            {
                let _ = fs::remove_file(&path);
                return Err(error);
            }
            Ok(path)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_file() => Ok(path),
                Ok(_) => Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "edit draft is not a file",
                )),
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{create_edit_file, edit_draft_path, has_edit_draft};

    #[test]
    fn edit_drafts_are_reused_after_an_interrupted_session() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        fs::write(&path, b"original").unwrap();
        let draft = create_edit_file(&path, b"original").unwrap();
        fs::write(&draft, b"pending").unwrap();
        let reused = create_edit_file(&path, b"new baseline").unwrap();
        assert_eq!(reused, draft);
        assert_eq!(fs::read(&reused).unwrap(), b"pending");
        assert!(has_edit_draft(&path));
        assert_eq!(
            draft.file_name().unwrap(),
            Path::new(".config.cprof-edit.toml").file_name().unwrap()
        );
    }

    #[test]
    fn edit_draft_path_supports_extensionless_and_hidden_names() {
        assert_eq!(
            edit_draft_path(Path::new("/profiles/settings"))
                .file_name()
                .unwrap()
                .to_string_lossy(),
            ".settings.cprof-edit"
        );
        assert_eq!(
            edit_draft_path(Path::new("/profiles/.credentials"))
                .file_name()
                .unwrap()
                .to_string_lossy(),
            "..credentials.cprof-edit"
        );
    }
}
