use std::fs;
#[cfg(unix)]
use std::fs::File;
#[cfg(windows)]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::os::windows::fs::symlink_file;
use std::path::Path;

use crate::error::{IoContext, Result};

#[cfg(unix)]
use super::parent;

pub(super) fn sync_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).with_path(path)?;
    // Links may point to a profile directory that has not been committed yet.
    // Syncing their parent persists the entry; opening them follows the target.
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    #[cfg(windows)]
    {
        if metadata.is_dir() {
            return Ok(());
        }
        // FlushFileBuffers needs a writable handle. Only adjust the staged
        // copy's readonly attribute, never the original or a link's target.
        let permissions = metadata.permissions();
        if permissions.readonly() {
            let mut writable = permissions.clone();
            // This branch is Windows-only: clears FILE_ATTRIBUTE_READONLY,
            // not Unix permission bits.
            #[allow(clippy::permissions_set_readonly_false)]
            writable.set_readonly(false);
            fs::set_permissions(path, writable).with_path(path)?;
        }
        let result = OpenOptions::new()
            .write(true)
            .open(path)
            .with_path(path)
            .and_then(|file| file.sync_all().with_path(path));
        let restore = if permissions.readonly() {
            fs::set_permissions(path, permissions).with_path(path)
        } else {
            Ok(())
        };
        match result {
            Ok(()) => restore,
            Err(error) => Err(error.with_recovery(restore.err())),
        }
    }
    #[cfg(not(windows))]
    {
        File::open(path).with_path(path)?.sync_all().with_path(path)
    }
}

#[cfg(unix)]
pub(super) fn sync_parent(path: &Path) -> Result<()> {
    let parent = parent(path)?;
    File::open(parent)
        .with_path(parent)?
        .sync_all()
        .with_path(parent)?;
    Ok(())
}

#[cfg(windows)]
pub(super) fn sync_parent(_: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
pub(crate) fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    symlink(target, link).with_path(link)?;
    Ok(())
}

#[cfg(windows)]
pub(crate) fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    symlink_file(target, link).with_path(link)?;
    Ok(())
}
