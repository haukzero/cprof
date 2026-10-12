use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use crate::error::{AppError, IoContext, Result, TransactionError};

use super::platform::{create_symlink, sync_parent, sync_path};
use super::{
    create_edit_file, create_staged_file, edit_draft_path, parent, path_exists,
    remove_any_if_exists, temporary_sibling,
};

fn transaction_conflict(path: &Path, expected_existing: bool) -> AppError {
    let expectation = if expected_existing {
        "to still exist"
    } else {
        "to remain absent"
    };
    TransactionError::Conflict(format!("expected '{}' {expectation}", path.display())).into()
}

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

    /// Stage a group of changes, reporting cleanup failures if preparation fails.
    pub(crate) fn prepare(stage: impl FnOnce(&mut Self) -> Result<()>) -> Result<Self> {
        let mut transaction = Self::new();
        match stage(&mut transaction) {
            Ok(()) => Ok(transaction),
            Err(error) => Err(transaction.cancel(error)),
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
        self.add_operation(destination, PathChange::Write(staging.clone()), replace)?;
        Ok(staging)
    }

    pub(crate) fn stage_edit_file(
        &mut self,
        destination: &Path,
        content: &[u8],
        replace: bool,
    ) -> Result<PathBuf> {
        let staging = create_edit_file(destination, content)
            .map_err(|source| AppError::io(&edit_draft_path(destination), source))?;
        self.add_operation(destination, PathChange::Write(staging.clone()), replace)?;
        Ok(staging)
    }

    pub(crate) fn stage_directory(&mut self, destination: &Path, replace: bool) -> Result<PathBuf> {
        let parent = parent(destination)?;
        fs::create_dir_all(parent).with_path(parent)?;
        let staging = loop {
            let path = temporary_sibling(destination, "stage");
            let builder = &mut fs::DirBuilder::new();
            // Profiles can contain credentials, including while staged.
            #[cfg(unix)]
            builder.mode(0o700);
            match builder.create(&path) {
                Ok(()) => break path,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(AppError::io(&path, error)),
            }
        };
        self.add_operation(destination, PathChange::Write(staging.clone()), replace)?;
        Ok(staging)
    }

    pub(crate) fn stage_symlink(
        &mut self,
        destination: &Path,
        source: &Path,
        replace: bool,
    ) -> Result<()> {
        let parent = parent(destination)?;
        fs::create_dir_all(parent).with_path(parent)?;
        let staging = loop {
            let path = temporary_sibling(destination, "stage");
            match create_symlink(source, &path) {
                Ok(()) => break path,
                Err(error) if error.is_io_kind(io::ErrorKind::AlreadyExists) => continue,
                Err(error) => return Err(error),
            }
        };
        self.add_operation(destination, PathChange::Write(staging), replace)
    }

    pub(crate) fn stage_remove(&mut self, destination: &Path) -> Result<()> {
        self.add_operation(destination, PathChange::Remove, true)
    }

    /// Keep the original in place until commit; rollback moves it back intact.
    pub(crate) fn stage_rename(&mut self, source: &Path, destination: &Path) -> Result<()> {
        if !path_exists(source)? {
            return Err(transaction_conflict(source, true));
        }
        self.add_operation(destination, PathChange::Rename(source.to_path_buf()), false)
    }

    pub(crate) fn discard_last(&mut self) -> Result<()> {
        let operation = self.operations.pop().expect("a staged operation exists");
        operation.clear_staging()
    }

    fn add_operation(
        &mut self,
        destination: &Path,
        change: PathChange,
        replace: bool,
    ) -> Result<()> {
        let operation = PathOperation {
            destination: destination.to_path_buf(),
            rollback: temporary_sibling(destination, "rollback"),
            change,
            replace,
            backed_up: false,
            applied: false,
        };
        let check = path_exists(destination).and_then(|exists| {
            if exists == replace {
                Ok(())
            } else {
                Err(transaction_conflict(destination, replace))
            }
        });
        if let Err(error) = check {
            return Err(error.with_recovery(operation.clear_staging().err()));
        }
        self.operations.push(operation);
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
            Err(TransactionError::CleanupFailed(cleanup_errors).into())
        }
    }

    pub(crate) fn cancel(mut self, error: AppError) -> AppError {
        self.recover(error)
    }

    fn recover(&mut self, error: AppError) -> AppError {
        let recovery = self.rollback_all();
        self.settled = true;
        error.with_recovery(recovery)
    }

    fn rollback_all(&mut self) -> Vec<AppError> {
        let mut errors = Vec::new();
        for operation in self.operations.iter_mut().rev() {
            errors.extend(operation.rollback());
        }
        if let Err(error) = self.sync_parents() {
            errors.push(error);
        }
        errors
    }

    fn sync_parents(&self) -> Result<()> {
        let mut synced = Vec::new();
        for operation in &self.operations {
            let origin = match &operation.change {
                PathChange::Rename(path) => Some(path),
                _ => None,
            };
            for path in std::iter::once(&operation.destination).chain(origin) {
                let parent = parent(path)?;
                if !synced.contains(&parent) {
                    sync_parent(path)?;
                    synced.push(parent);
                }
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
    change: PathChange,
    rollback: PathBuf,
    replace: bool,
    backed_up: bool,
    applied: bool,
}

enum PathChange {
    Write(PathBuf),
    Remove,
    Rename(PathBuf),
}

impl PathOperation {
    fn apply(&mut self) -> Result<()> {
        if path_exists(&self.destination)? != self.replace {
            return Err(transaction_conflict(&self.destination, self.replace));
        }
        if path_exists(&self.rollback)? {
            return Err(TransactionError::Conflict(format!(
                "Transaction rollback path '{}' already exists",
                self.rollback.display()
            ))
            .into());
        }

        match &self.change {
            PathChange::Write(path) => sync_path(path)?,
            PathChange::Rename(path) if !path_exists(path)? => {
                return Err(transaction_conflict(path, true));
            }
            _ => {}
        }
        if self.replace {
            if let Err(error) = fs::rename(&self.destination, &self.rollback) {
                return match path_exists(&self.destination) {
                    Ok(false) => Err(transaction_conflict(&self.destination, true)),
                    _ => Err(AppError::io(&self.destination, error)),
                };
            }
            self.backed_up = true;
        }
        if let PathChange::Write(source) | PathChange::Rename(source) = &self.change {
            if let Err(error) = fs::rename(source, &self.destination) {
                return match path_exists(&self.destination) {
                    Ok(exists) if exists != self.replace => {
                        Err(transaction_conflict(&self.destination, self.replace))
                    }
                    _ => Err(AppError::io(&self.destination, error)),
                };
            }
            self.applied = true;
        }
        Ok(())
    }

    fn rollback(&mut self) -> Vec<AppError> {
        let mut errors = Vec::new();
        if self.applied {
            let recovery = match &self.change {
                PathChange::Rename(path) => path_exists(path).and_then(|exists| {
                    if exists {
                        return Err(transaction_conflict(path, false));
                    }
                    fs::rename(&self.destination, path).with_path(path)
                }),
                _ => remove_any_if_exists(&self.destination),
            };
            match recovery {
                Ok(()) => self.applied = false,
                Err(error) => errors.push(error),
            }
        }
        if self.backed_up {
            match fs::rename(&self.rollback, &self.destination) {
                Ok(()) => self.backed_up = false,
                Err(error) => errors.push(AppError::io(&self.destination, error)),
            }
        }
        if let Err(error) = self.clear_staging() {
            errors.push(error);
        }
        errors
    }

    fn finish(&mut self) -> Result<()> {
        if self.backed_up {
            remove_any_if_exists(&self.rollback)?;
            self.backed_up = false;
        }
        Ok(())
    }

    fn clear_staging(&self) -> Result<()> {
        match &self.change {
            PathChange::Write(path) => remove_any_if_exists(path),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs;
    use std::io;
    use std::path::Path;

    use crate::error::{AppError, TransactionError};

    use super::{PathTransaction, temporary_sibling};

    #[test]
    fn staging_siblings_keep_the_destination_extension() {
        let temporary = temporary_sibling(Path::new("/profiles/config.toml"), "stage");

        assert_eq!(temporary.extension(), Some(OsStr::new("toml")));
        let name = temporary.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with(".config.cprof-stage-"), "{name}");
        assert!(name.ends_with(".toml"), "{name}");
    }

    #[test]
    fn staging_siblings_support_extensionless_and_hidden_names() {
        let extensionless = temporary_sibling(Path::new("/profiles/settings"), "stage");
        let hidden = temporary_sibling(Path::new("/profiles/.credentials"), "stage");

        assert!(
            extensionless
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".settings.cprof-stage-")
        );
        assert!(
            hidden
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("..credentials.cprof-stage-")
        );
    }

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

    #[test]
    fn removal_is_reversible_until_the_transaction_commits() {
        let directory = tempfile::tempdir().unwrap();
        let removed = directory.path().join("optional");
        let conflict = directory.path().join("conflict");
        fs::write(&removed, "original").unwrap();

        let transaction = PathTransaction::prepare(|transaction| {
            transaction.stage_remove(&removed)?;
            transaction.stage_file(&conflict, "stage", b"new", false)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(fs::read_to_string(&removed).unwrap(), "original");
        fs::write(&conflict, "concurrent").unwrap();
        assert!(matches!(
            transaction.commit(),
            Err(AppError::Transaction(TransactionError::Conflict(_)))
        ));
        assert_eq!(fs::read_to_string(&removed).unwrap(), "original");

        PathTransaction::prepare(|transaction| transaction.stage_remove(&removed))
            .unwrap()
            .commit()
            .unwrap();
        assert!(!removed.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        assert_eq!(fs::read_to_string(conflict).unwrap(), "concurrent");
    }

    #[test]
    fn staged_rename_leaves_the_original_in_place_on_cancel_or_drop() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("old");
        let destination = directory.path().join("new");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("unlisted file"), "original").unwrap();

        let transaction =
            PathTransaction::prepare(|transaction| transaction.stage_rename(&source, &destination))
                .unwrap();
        assert!(source.is_dir());
        assert!(!destination.exists());
        drop(transaction);
        assert_eq!(
            fs::read_to_string(source.join("unlisted file")).unwrap(),
            "original"
        );

        let result = PathTransaction::prepare(|transaction| {
            transaction.stage_rename(&source, &destination)?;
            transaction.stage_remove(&directory.path().join("missing"))
        });
        assert!(result.is_err());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        assert_eq!(
            fs::read_to_string(source.join("unlisted file")).unwrap(),
            "original"
        );
    }

    #[test]
    fn rename_conflicts_preserve_both_original_paths() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("old");
        let destination = directory.path().join("new");
        fs::write(&source, "original").unwrap();
        let transaction =
            PathTransaction::prepare(|transaction| transaction.stage_rename(&source, &destination))
                .unwrap();
        fs::write(&destination, "concurrent").unwrap();
        assert!(transaction.commit().is_err());
        assert_eq!(fs::read_to_string(&source).unwrap(), "original");
        assert_eq!(fs::read_to_string(&destination).unwrap(), "concurrent");
        assert!(
            PathTransaction::prepare(|transaction| transaction.stage_rename(&source, &destination))
                .is_err()
        );
        assert_eq!(fs::read_to_string(&source).unwrap(), "original");
        assert_eq!(fs::read_to_string(&destination).unwrap(), "concurrent");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[cprof_macros::windows_elevation]
    #[test]
    fn rename_and_link_changes_recover_together_after_a_later_conflict() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("old");
        let destination = directory.path().join("new");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("settings"), "original").unwrap();
        fs::write(source.join("unlisted file"), "retained").unwrap();
        let link = directory.path().join("active");
        let relative = Path::new("old").join("settings");
        crate::filesystem::create_symlink(&relative, &link).unwrap();
        let conflict = directory.path().join("conflict");
        let transaction = PathTransaction::prepare(|transaction| {
            transaction.stage_rename(&source, &destination)?;
            transaction.stage_symlink(&link, &destination.join("settings"), true)?;
            transaction.stage_file(&conflict, "stage", b"incoming", false)?;
            Ok(())
        })
        .unwrap();
        fs::write(&conflict, "concurrent").unwrap();

        assert!(transaction.commit().is_err());
        assert!(!destination.exists());
        assert_eq!(fs::read_link(&link).unwrap(), relative);
        assert_eq!(fs::read_to_string(&link).unwrap(), "original");
        assert_eq!(
            fs::read_to_string(source.join("unlisted file")).unwrap(),
            "retained"
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 3);

        PathTransaction::prepare(|transaction| {
            transaction.stage_rename(&source, &destination)?;
            transaction.stage_symlink(&link, &destination.join("settings"), true)
        })
        .unwrap()
        .commit()
        .unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read_to_string(&link).unwrap(), "original");
        assert_eq!(
            fs::read_to_string(destination.join("unlisted file")).unwrap(),
            "retained"
        );
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 3);
    }

    #[test]
    fn rename_recovery_preserves_an_unexpected_entry_at_the_original_path() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("old");
        let destination = directory.path().join("new");
        fs::write(&source, "original").unwrap();
        let mut transaction =
            PathTransaction::prepare(|transaction| transaction.stage_rename(&source, &destination))
                .unwrap();
        transaction.operations[0].apply().unwrap();
        fs::write(&source, "concurrent").unwrap();

        let error = transaction.cancel(TransactionError::Conflict("later failure".into()).into());
        assert!(matches!(
            error,
            AppError::Transaction(TransactionError::RecoveryFailed { .. })
        ));
        assert_eq!(fs::read_to_string(&source).unwrap(), "concurrent");
        assert_eq!(fs::read_to_string(&destination).unwrap(), "original");
    }

    #[cprof_macros::windows_elevation]
    #[test]
    fn batch_removal_restores_directories_links_and_configuration_on_failure() {
        let directory = tempfile::tempdir().unwrap();
        let profiles = directory.path().join("profiles");
        fs::create_dir(&profiles).unwrap();
        fs::write(profiles.join("settings"), "original").unwrap();
        let link = directory.path().join("active");
        let relative = Path::new("profiles").join("settings");
        crate::filesystem::create_symlink(&relative, &link).unwrap();
        let config = directory.path().join("extra-target.toml");
        fs::write(&config, "[demo]\n").unwrap();
        let conflict = directory.path().join("conflict");
        let transaction = PathTransaction::prepare(|transaction| {
            transaction.stage_remove(&link)?;
            transaction.stage_remove(&profiles)?;
            transaction.stage_remove(&config)?;
            transaction.stage_file(&conflict, "stage", b"incoming", false)?;
            Ok(())
        })
        .unwrap();
        fs::write(&conflict, "concurrent").unwrap();

        assert!(transaction.commit().is_err());
        assert_eq!(fs::read_link(&link).unwrap(), relative);
        assert_eq!(fs::read_to_string(&link).unwrap(), "original");
        assert_eq!(fs::read_to_string(&config).unwrap(), "[demo]\n");
        assert_eq!(fs::read_to_string(&conflict).unwrap(), "concurrent");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 4);
    }

    #[test]
    fn rollback_failure_keeps_the_primary_error_and_recovery_errors() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config");
        fs::write(&path, "original").unwrap();
        let mut transaction = PathTransaction::prepare(|transaction| {
            transaction.stage_file(&path, "stage", b"replacement", true)?;
            Ok(())
        })
        .unwrap();
        transaction.operations[0].apply().unwrap();
        // Simulate a backup disappearing before the operation can recover.
        fs::remove_file(&transaction.operations[0].rollback).unwrap();
        let error = transaction.cancel(TransactionError::Conflict("primary failure".into()).into());
        let AppError::Transaction(TransactionError::RecoveryFailed { source, errors }) = error
        else {
            panic!("expected a recovery failure")
        };
        assert!(
            matches!(*source, AppError::Transaction(TransactionError::Conflict(ref message)) if message == "primary failure")
        );
        assert_eq!(errors.len(), 1);
        assert!(errors[0].is_io_kind(io::ErrorKind::NotFound));
    }
}
