use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// An OS-backed exclusive lock for one cprof edit scope.
#[derive(Debug)]
pub(crate) struct EditLock {
    _file: File,
}

impl EditLock {
    pub(crate) fn try_acquire(scope: &Path) -> io::Result<Option<Self>> {
        let path = lock_path(scope);
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(error)) => Err(error),
        }
    }

    pub(crate) fn path(scope: &Path) -> PathBuf {
        lock_path(scope)
    }
}

fn lock_path(scope: &Path) -> PathBuf {
    let digest = Sha256::digest(scope.as_os_str().as_encoded_bytes());
    std::env::temp_dir().join(format!("cprof-edit-{}.lock", hex(&digest[..12])))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::EditLock;

    #[test]
    fn lock_is_exclusive_and_released_on_drop() {
        let directory = tempfile::tempdir().unwrap();
        let scope = directory.path().join("profile");
        let other_scope = directory.path().join("other-profile");
        let first = EditLock::try_acquire(&scope).unwrap().unwrap();
        assert!(EditLock::try_acquire(&scope).unwrap().is_none());
        assert!(EditLock::try_acquire(&other_scope).unwrap().is_some());
        drop(first);
        assert!(EditLock::try_acquire(&scope).unwrap().is_some());
    }
}
