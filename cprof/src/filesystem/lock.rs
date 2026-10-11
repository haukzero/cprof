use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::config::paths;
use crate::error::{AppError, IoContext, Result, TransactionError};

/// OS-backed write locks, held until the operation and its recovery finish.
/// Lock files live outside paths that may be replaced or deleted.
#[derive(Debug)]
pub(crate) struct WriteLocks {
    files: Vec<File>,
}

impl WriteLocks {
    pub(crate) fn acquire(scopes: impl IntoIterator<Item = PathBuf>) -> Result<Self> {
        let mut locks = Self { files: Vec::new() };
        locks.acquire_more(scopes)?;
        Ok(locks)
    }

    /// Extend a session without waiting. The caller drops it on acquisition failure.
    pub(crate) fn acquire_more(&mut self, scopes: impl IntoIterator<Item = PathBuf>) -> Result<()> {
        let scopes = scopes
            .into_iter()
            .map(|scope| paths::resolve_existing(&scope))
            .collect::<Result<BTreeSet<_>>>()?;
        for scope in scopes {
            let path = lock_path(&scope);
            let file = OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(false)
                .open(&path)
                .with_path(&path)?;
            match file.try_lock() {
                Ok(()) => self.files.push(file),
                Err(std::fs::TryLockError::WouldBlock) => {
                    return Err(TransactionError::Locked { scope, lock: path }.into());
                }
                Err(std::fs::TryLockError::Error(error)) => {
                    return Err(AppError::io(&path, error));
                }
            }
        }
        Ok(())
    }
}

fn lock_path(scope: &Path) -> PathBuf {
    let digest = Sha256::digest(scope.as_os_str().as_encoded_bytes());
    std::env::temp_dir().join(format!("cprof-write-{}.lock", hex(&digest[..12])))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use crate::error::{AppError, TransactionError};

    use super::WriteLocks;

    #[test]
    fn lock_is_exclusive_and_released_on_drop() {
        let directory = tempfile::tempdir().unwrap();
        let scope = directory.path().join("profile");
        let other_scope = directory.path().join("other-profile");
        let first = WriteLocks::acquire([scope.clone()]).unwrap();
        assert!(matches!(
            WriteLocks::acquire([scope.clone()]),
            Err(AppError::Transaction(TransactionError::Locked { .. }))
        ));
        assert!(WriteLocks::acquire([other_scope]).is_ok());
        drop(first);
        assert!(WriteLocks::acquire([scope]).is_ok());
    }

    #[test]
    fn failed_batch_releases_locks_acquired_before_the_conflict() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("a");
        let busy = directory.path().join("b");
        let _busy = WriteLocks::acquire([busy.clone()]).unwrap();
        assert!(WriteLocks::acquire([first.clone(), busy]).is_err());
        assert!(WriteLocks::acquire([first]).is_ok());
    }

    #[cprof_macros::windows_elevation]
    #[test]
    fn parent_aliases_and_duplicate_scopes_share_one_lock() {
        let directory = tempfile::tempdir().unwrap();
        let original = directory.path().join("original");
        let alias = directory.path().join("alias");
        std::fs::create_dir(&original).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&original, &alias).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&original, &alias).unwrap();
        let scope = original.join("missing-target");
        let alias_scope = alias.join("missing-target");
        let locks = WriteLocks::acquire([scope.clone(), alias_scope.clone()]).unwrap();
        assert!(WriteLocks::acquire([alias_scope]).is_err());
        drop(locks);
        assert!(WriteLocks::acquire([scope]).is_ok());
    }
}
