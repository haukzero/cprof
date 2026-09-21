//! Filesystem primitives shared by profile, package, and activation code.
//!
//! Transaction orchestration lives in [`transaction`], while flushing and
//! symlink creation are isolated in [`platform`].

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{AppError, IoContext, PathError, Result};

mod platform;
pub(crate) mod transaction;

use platform::sync_path;
use transaction::PathTransaction;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let mut transaction = PathTransaction::new();
    transaction.stage_file(path, "stage", content, path_exists(path)?)?;
    transaction.commit()
}

pub(crate) fn write_file(path: &Path, content: &[u8]) -> Result<()> {
    let mut file = File::create(path).with_path(path)?;
    write_and_sync(&mut file, content).with_path(path)?;
    Ok(())
}

/// Use the platform copy operation to retain Unix permission bits and Windows
/// file attributes, without retaining a source symlink.
pub(crate) fn copy_file(source: &Path, destination: &Path) -> Result<()> {
    fs::copy(source, destination).with_path(destination)?;
    sync_path(destination)
}

#[cfg(test)]
pub(crate) fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    platform::create_symlink(target, link)
}

fn write_and_sync(file: &mut File, content: &[u8]) -> io::Result<()> {
    write_and_flush(file, content)?;
    file.sync_all()
}

fn write_and_flush(file: &mut File, content: &[u8]) -> io::Result<()> {
    file.write_all(content)?;
    file.flush()
}

fn create_staged_file(destination: &Path, label: &str, content: &[u8]) -> Result<PathBuf> {
    let parent = parent(destination)?;
    fs::create_dir_all(parent).with_path(parent)?;
    loop {
        let path = temporary_sibling(destination, label);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = write_and_flush(&mut file, content) {
                    let _ = fs::remove_file(&path);
                    return Err(AppError::io(&path, error));
                }
                if let Ok(metadata) = fs::metadata(destination)
                    && let Err(error) = fs::set_permissions(&path, metadata.permissions())
                {
                    let _ = fs::remove_file(&path);
                    return Err(AppError::io(&path, error));
                }
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(AppError::io(&path, error)),
        }
    }
}

fn parent(path: &Path) -> Result<&Path> {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => Ok(parent),
        Some(_) => Ok(Path::new(".")),
        None => Err(PathError::Unsafe(path.display().to_string()).into()),
    }
}

fn temporary_sibling(destination: &Path, label: &str) -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut name = OsString::from(".");
    name.push(
        destination
            .file_stem()
            .unwrap_or_else(|| destination.file_name().unwrap_or(destination.as_os_str())),
    );
    name.push(format!(".cprof-{label}-{}-{sequence}", process::id()));
    if let Some(extension) = destination.extension() {
        name.push(".");
        name.push(extension);
    }
    destination.with_file_name(name)
}

pub(crate) fn path_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AppError::io(path, error)),
    }
}

fn remove_any_if_exists(path: &Path) -> Result<()> {
    match remove_any(path) {
        Ok(()) => Ok(()),
        Err(error) if error.is_io_kind(io::ErrorKind::NotFound) => Ok(()),
        Err(error) => Err(error),
    }
}

fn remove_any(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).with_path(path)?;
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path).with_path(path)?;
    } else {
        fs::remove_file(path).with_path(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::atomic_write;

    #[test]
    fn atomic_write_commits_new_and_existing_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("output");

        atomic_write(&path, b"first").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"first");

        atomic_write(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
    }
}
