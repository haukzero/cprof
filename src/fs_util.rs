use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{AppError, Result};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) struct PathTransaction {
    operations: Vec<PathOperation>,
    settled: bool,
}

impl PathTransaction {
    pub(crate) fn new() -> Self {
        Self {
            operations: Vec::new(),
            settled: false,
        }
    }

    pub(crate) fn stage_file(
        &mut self,
        destination: &Path,
        label: &str,
        content: &[u8],
        replace: bool,
    ) -> Result<PathBuf> {
        let staging = create_staged_file(destination, label, content)?;
        self.add_operation(destination, staging.clone(), replace)?;
        Ok(staging)
    }

    pub(crate) fn stage_directory(&mut self, destination: &Path, replace: bool) -> Result<PathBuf> {
        let parent = parent(destination)?;
        fs::create_dir_all(parent)?;
        let staging = loop {
            let path = temporary_sibling(destination, "stage");
            match fs::create_dir(&path) {
                Ok(()) => break path,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        };
        self.add_operation(destination, staging.clone(), replace)?;
        Ok(staging)
    }

    pub(crate) fn discard_last(&mut self) -> Result<()> {
        let operation = self.operations.pop().expect("a staged operation exists");
        remove_any_if_exists(&operation.staging)
    }

    fn add_operation(&mut self, destination: &Path, staging: PathBuf, replace: bool) -> Result<()> {
        let exists = match path_exists(destination) {
            Ok(exists) => exists,
            Err(error) => {
                return Err(with_cleanup_error(error.into(), remove_any(&staging).err()));
            }
        };
        if exists != replace {
            return Err(with_cleanup_error(
                transaction_conflict(destination, replace),
                remove_any(&staging).err(),
            ));
        }
        self.operations.push(PathOperation {
            destination: destination.to_path_buf(),
            rollback: temporary_sibling(destination, "rollback"),
            staging,
            replace,
            backed_up: false,
            applied: false,
        });
        Ok(())
    }

    pub(crate) fn commit(mut self) -> Result<()> {
        for index in 0..self.operations.len() {
            if let Err(error) = self.operations[index].apply() {
                return Err(self.recover(error));
            }
        }

        if let Err(error) = self.sync_parents() {
            return Err(self.recover(error));
        }

        let mut cleanup_errors = Vec::new();
        for operation in &mut self.operations {
            if let Err(error) = operation.finish() {
                cleanup_errors.push(error);
            }
        }
        if let Err(error) = self.sync_parents() {
            cleanup_errors.push(error);
        }
        self.settled = true;
        if cleanup_errors.is_empty() {
            Ok(())
        } else {
            Err(AppError::Other(format!(
                "Transaction committed, but cleanup failed: {}",
                format_errors(&cleanup_errors)
            )))
        }
    }

    pub(crate) fn cancel(mut self, error: AppError) -> AppError {
        self.recover(error)
    }

    fn recover(&mut self, error: AppError) -> AppError {
        let recovery = self.rollback_all();
        self.settled = true;
        with_recovery_errors(error, recovery)
    }

    fn rollback_all(&mut self) -> Vec<AppError> {
        let mut errors = Vec::new();
        for operation in self.operations.iter_mut().rev() {
            if let Err(error) = operation.rollback() {
                errors.push(error);
            }
        }
        if let Err(error) = self.sync_parents() {
            errors.push(error);
        }
        errors
    }

    fn sync_parents(&self) -> Result<()> {
        let mut synced = Vec::new();
        for operation in &self.operations {
            let parent = parent(&operation.destination)?;
            if !synced.contains(&parent) {
                sync_parent(&operation.destination)?;
                synced.push(parent);
            }
        }
        Ok(())
    }
}

impl Drop for PathTransaction {
    fn drop(&mut self) {
        if self.settled {
            return;
        }
        self.rollback_all();
    }
}

struct PathOperation {
    destination: PathBuf,
    staging: PathBuf,
    rollback: PathBuf,
    replace: bool,
    backed_up: bool,
    applied: bool,
}

impl PathOperation {
    fn apply(&mut self) -> Result<()> {
        if path_exists(&self.destination)? != self.replace {
            return Err(transaction_conflict(&self.destination, self.replace));
        }
        if path_exists(&self.rollback)? {
            return Err(AppError::Other(format!(
                "Transaction rollback path '{}' already exists",
                self.rollback.display()
            )));
        }

        sync_path(&self.staging)?;
        if self.replace {
            if let Err(error) = fs::rename(&self.destination, &self.rollback) {
                return match path_exists(&self.destination) {
                    Ok(false) => Err(transaction_conflict(&self.destination, true)),
                    _ => Err(error.into()),
                };
            }
            self.backed_up = true;
        }
        if let Err(error) = fs::rename(&self.staging, &self.destination) {
            return match path_exists(&self.destination) {
                Ok(exists) if exists != self.replace => {
                    Err(transaction_conflict(&self.destination, self.replace))
                }
                _ => Err(error.into()),
            };
        }
        self.applied = true;
        Ok(())
    }

    fn rollback(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        if self.applied {
            match remove_any_if_exists(&self.destination) {
                Ok(()) => self.applied = false,
                Err(error) => errors.push(error),
            }
        }
        if self.backed_up {
            match fs::rename(&self.rollback, &self.destination) {
                Ok(()) => self.backed_up = false,
                Err(error) => errors.push(error.into()),
            }
        }
        if let Err(error) = remove_any_if_exists(&self.staging) {
            errors.push(error);
        }
        errors_result(errors)
    }

    fn finish(&mut self) -> Result<()> {
        if self.backed_up {
            remove_any_if_exists(&self.rollback)?;
            self.backed_up = false;
        }
        Ok(())
    }
}

pub(crate) fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let mut transaction = PathTransaction::new();
    transaction.stage_file(path, "stage", content, path_exists(path)?)?;
    transaction.commit()
}

pub fn write_file(path: &Path, content: &[u8]) -> Result<()> {
    let mut file = File::create(path)?;
    write_and_sync(&mut file, content)?;
    Ok(())
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
    fs::create_dir_all(parent(destination)?)?;
    loop {
        let path = temporary_sibling(destination, label);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = write_and_flush(&mut file, content) {
                    let _ = fs::remove_file(&path);
                    return Err(error.into());
                }
                if let Ok(metadata) = fs::metadata(destination)
                    && let Err(error) = fs::set_permissions(&path, metadata.permissions())
                {
                    let _ = fs::remove_file(&path);
                    return Err(error.into());
                }
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

fn parent(path: &Path) -> Result<&Path> {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => Ok(parent),
        Some(_) => Ok(Path::new(".")),
        None => Err(AppError::Other(format!(
            "Path '{}' has no parent directory",
            path.display()
        ))),
    }
}

fn temporary_sibling(destination: &Path, label: &str) -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut name = OsString::from(".");
    name.push(destination.file_name().unwrap_or(destination.as_os_str()));
    name.push(format!(".cprof-{label}-{}-{sequence}", std::process::id()));
    destination.with_file_name(name)
}

fn path_exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn transaction_conflict(path: &Path, expected_existing: bool) -> AppError {
    let expectation = if expected_existing {
        "to still exist"
    } else {
        "to remain absent"
    };
    AppError::TransactionConflict(format!("expected '{}' {expectation}", path.display()))
}

fn remove_any_if_exists(path: &Path) -> Result<()> {
    match remove_any(path) {
        Ok(()) => Ok(()),
        Err(AppError::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn remove_any(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn sync_path(path: &Path) -> Result<()> {
    #[cfg(windows)]
    if fs::metadata(path)?.is_dir() {
        return Ok(());
    }
    File::open(path)?.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<()> {
    File::open(parent(path)?)?.sync_all()?;
    Ok(())
}

#[cfg(windows)]
fn sync_parent(_: &Path) -> Result<()> {
    Ok(())
}

fn with_recovery_errors(primary: AppError, recovery: Vec<AppError>) -> AppError {
    if recovery.is_empty() {
        primary
    } else {
        AppError::Other(format!(
            "{primary}; rollback or cleanup also failed: {}",
            format_errors(&recovery)
        ))
    }
}

fn with_cleanup_error(primary: AppError, cleanup: Option<AppError>) -> AppError {
    match cleanup {
        Some(cleanup) => AppError::Other(format!("{primary}; cleanup also failed: {cleanup}")),
        None => primary,
    }
}

fn format_errors(errors: &[AppError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

fn errors_result(errors: Vec<AppError>) -> Result<()> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(AppError::Other(format_errors(&errors)))
    }
}

#[cfg(unix)]
pub fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

#[cfg(windows)]
pub fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    std::os::windows::fs::symlink_file(target, link)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::PathTransaction;
    use std::fs;

    #[test]
    fn commit_conflict_restores_already_applied_paths() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        fs::write(&first, "original").unwrap();

        let mut transaction = PathTransaction::new();
        transaction
            .stage_file(&first, "stage", b"replacement", true)
            .unwrap();
        transaction
            .stage_file(&second, "stage", b"incoming", false)
            .unwrap();
        fs::write(&second, "concurrent").unwrap();

        assert!(transaction.commit().is_err());
        assert_eq!(fs::read_to_string(first).unwrap(), "original");
        assert_eq!(fs::read_to_string(second).unwrap(), "concurrent");
    }
}
