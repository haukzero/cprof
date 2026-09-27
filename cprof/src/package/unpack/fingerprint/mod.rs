use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::config;
use crate::error::{IoContext, PackageError, PathError, Result};
use crate::package::PackageProfile;
use crate::profile::storage;
use crate::targets::{TargetRepository, TargetSpec};

use super::UnpackMode;

/// Bind replayed decisions to the package and the local state they described.
pub(super) struct Fingerprint(Sha256);

impl Fingerprint {
    pub(super) fn new(
        package: &[u8],
        targets: &TargetRepository,
        selected: Option<&str>,
        mode: UnpackMode,
    ) -> Result<Self> {
        let mut fingerprint = Self(Sha256::new());
        fingerprint.add(package);
        fingerprint.add(selected.unwrap_or_default().as_bytes());
        fingerprint.add(&[u8::from(mode == UnpackMode::Mirror)]);
        fingerprint.add(
            &serde_json::to_vec(targets.external_configs())
                .map_err(|error| PackageError::Invalid(error.to_string()))?,
        );
        fingerprint.path(&config::extra_target_file()?)?;
        Ok(fingerprint)
    }

    pub(super) fn target(
        &mut self,
        target: &TargetSpec,
        profiles: &[PackageProfile],
        mode: UnpackMode,
    ) -> Result<()> {
        self.add(target.id.as_bytes());
        let mut names = profiles
            .iter()
            .map(|profile| profile.name.clone())
            .collect::<BTreeSet<_>>();
        if mode == UnpackMode::Mirror {
            names.extend(storage::names(target)?);
            for resource in &target.resources {
                self.path(&config::active_resource(target, resource)?)?;
            }
        }
        // Rollback can leave empty parent directories; only profile paths matter.
        for name in names {
            self.path(&config::profile_dir(target, &name)?)?;
        }
        Ok(())
    }

    pub(super) fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }

    fn add(&mut self, bytes: &[u8]) {
        self.0.update((bytes.len() as u64).to_le_bytes());
        self.0.update(bytes);
    }

    fn path(&mut self, path: &Path) -> Result<()> {
        self.add(path.as_os_str().as_encoded_bytes());
        let metadata = match fs::symlink_metadata(path) {
            Err(error) if error.kind() == ErrorKind::NotFound => {
                self.add(b"missing");
                return Ok(());
            }
            result => result.with_path(path)?,
        };
        if metadata.is_symlink() {
            self.add(b"link");
            self.add(
                fs::read_link(path)
                    .with_path(path)?
                    .as_os_str()
                    .as_encoded_bytes(),
            );
        } else if metadata.is_file() {
            self.add(b"file");
            self.add(&fs::read(path).with_path(path)?);
        } else if metadata.is_dir() {
            self.add(b"directory");
            let mut entries = fs::read_dir(path)
                .with_path(path)?
                .map(|entry| entry.map(|entry| entry.path()).with_path(path))
                .collect::<Result<Vec<_>>>()?;
            entries.sort();
            for entry in entries {
                self.path(&entry)?;
            }
            self.add(b"end");
        } else {
            return Err(PathError::Unsafe(path.display().to_string()).into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
