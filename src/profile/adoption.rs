//! Import unmanaged active resources as a new profile and activate it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::config::{self, paths};
use crate::error::{ActivationError, AppError, IoContext, ProfileError, Result, TargetError};
use crate::filesystem::{self, transaction::PathTransaction};
use crate::targets::{ResourceSpec, TargetSpec};

use super::activation::{self, Status};

/// Preserve the current configuration as a new profile and replace its active
/// entries with managed links. External link targets are only read/copied.
pub fn adopt(target: &TargetSpec, name: &str) -> Result<()> {
    let destination = config::profile_dir(target, name)?;
    if destination.exists() {
        return Err(ProfileError::Exists(name.to_string()).into());
    }
    match activation::status(target)? {
        Status::Unmanaged => {}
        Status::Active(active) => {
            return Err(ActivationError::AlreadyManaged {
                target: target.id.clone(),
                profile: active,
            }
            .into());
        }
        Status::NoFiles => {
            return Err(ActivationError::NoConfiguration(target.id.clone()).into());
        }
        Status::Partial | Status::Mixed => {
            return Err(ActivationError::InconsistentLinks(target.id.clone()).into());
        }
    }

    let sources = target
        .resources
        .iter()
        .map(|spec| ActiveResource::read(spec, config::active_resource(target, spec)?))
        .collect::<Result<Vec<_>>>()?;
    let transaction = stage_adoption(&destination, &sources)?;
    commit_adoption(transaction, &sources)
}

struct ActiveResource {
    filename: String,
    path: PathBuf,
    original: Option<FileSnapshot>,
}

impl ActiveResource {
    fn read(spec: &ResourceSpec, path: PathBuf) -> Result<Self> {
        paths::validate_filename(&spec.filename)?;
        let original = FileSnapshot::read(&path)?;
        match &original {
            Some(original) => (spec.validate)(&original.content).map_err(|error| {
                TargetError::ResourceValidation {
                    path: path.clone(),
                    source: Box::new(error),
                }
            })?,
            None if spec.required => {
                return Err(ActivationError::MissingResource {
                    resource: spec.key.clone(),
                    path,
                }
                .into());
            }
            None => {}
        }
        Ok(Self {
            filename: spec.filename.clone(),
            path,
            original,
        })
    }

    fn verify_unchanged(&self) -> Result<()> {
        if FileSnapshot::read(&self.path)? != self.original {
            return Err(ActivationError::ConfigurationChanged(self.path.clone()).into());
        }
        Ok(())
    }
}

#[derive(PartialEq)]
struct FileSnapshot {
    content: Vec<u8>,
    link_target: Option<PathBuf>,
    resolved_path: PathBuf,
    permissions: fs::Permissions,
    modified: Option<SystemTime>,
}

impl FileSnapshot {
    fn read(path: &Path) -> Result<Option<Self>> {
        let entry = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(AppError::io(path, error)),
        };
        let link_target = if entry.file_type().is_symlink() {
            Some(fs::read_link(path).with_path(path)?)
        } else {
            None
        };
        // metadata follows symlinks; a broken link is an error even for an
        // optional resource, and directories/devices must never be read.
        let metadata = fs::metadata(path).with_path(path)?;
        if !metadata.is_file() {
            return Err(TargetError::InvalidResource(format!(
                "'{}' does not resolve to a regular file",
                path.display()
            ))
            .into());
        }
        Ok(Some(Self {
            content: fs::read(path).with_path(path)?,
            link_target,
            resolved_path: fs::canonicalize(path).with_path(path)?,
            permissions: metadata.permissions(),
            modified: metadata.modified().ok(),
        }))
    }
}

fn stage_adoption(destination: &Path, sources: &[ActiveResource]) -> Result<PathTransaction> {
    PathTransaction::prepare(|transaction| {
        let staging = transaction.stage_directory(destination, false)?;
        for source in sources {
            let Some(original) = &source.original else {
                continue;
            };
            let filename = Path::new(&source.filename);
            let stored = paths::join_storage_under(&staging, filename)?;
            // Copy rather than serializing the snapshot: retain native file
            // permissions/attributes, then verify we copied the validated bytes.
            filesystem::copy_file(&source.path, &stored)?;
            if fs::read(&stored).with_path(&stored)? != original.content {
                return Err(ActivationError::ConfigurationChanged(source.path.clone()).into());
            }
            transaction.stage_symlink(&source.path, &destination.join(filename), true)?;
        }
        Ok(())
    })
}

fn commit_adoption(transaction: PathTransaction, sources: &[ActiveResource]) -> Result<()> {
    if let Err(error) = sources
        .iter()
        .try_for_each(ActiveResource::verify_unchanged)
    {
        return Err(transaction.cancel(error));
    }
    transaction.commit()
}

#[cfg(test)]
mod tests {
    use crate::elevate;

    use super::*;

    fn spec(filename: &str, required: bool) -> ResourceSpec {
        ResourceSpec {
            key: filename.to_string(),
            filename: filename.to_string(),
            active_path: None,
            absolute_active_path: None,
            required,
            template: Vec::new(),
            validate: |_| Ok(()),
        }
    }

    #[test]
    fn resource_errors_retain_the_path_and_validation_cause() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("auth.json");
        let resource = ResourceSpec {
            validate: |content| {
                serde_json::from_slice::<serde_json::Value>(content)
                    .map(|_| ())
                    .map_err(AppError::Json)
            },
            ..spec("auth.json", true)
        };
        assert!(matches!(ActiveResource::read(&resource, path.clone()),
            Err(AppError::Activation(ActivationError::MissingResource { path: missing, .. })) if missing == path));
        fs::write(&path, "{").unwrap();
        assert!(matches!(ActiveResource::read(&resource, path.clone()),
            Err(AppError::Target(TargetError::ResourceValidation { path: invalid, source }))
                if invalid == path && matches!(*source, AppError::Json(_))));
    }

    fn can_create_links(directory: &Path) -> bool {
        let link = directory.join("probe-link");
        match filesystem::create_symlink(&directory.join("missing"), &link) {
            Ok(()) => {
                fs::remove_file(link).unwrap();
                true
            }
            Err(error) if elevate::is_privilege_error(&error) => {
                eprintln!("Skipping symlink test: Windows Developer Mode or elevation required");
                false
            }
            Err(error) => panic!("{error}"),
        }
    }

    fn sources(directory: &Path) -> Vec<ActiveResource> {
        let active = directory.join("active");
        fs::create_dir(&active).unwrap();
        ["config", "auth"]
            .into_iter()
            .map(|filename| {
                let path = active.join(filename);
                fs::write(&path, filename).unwrap();
                ActiveResource::read(&spec(filename, true), path).unwrap()
            })
            .collect()
    }

    fn assert_no_staging(directory: &Path) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            assert!(!entry.file_name().to_string_lossy().contains(".cprof-"));
            if entry.file_type().unwrap().is_dir() {
                assert_no_staging(&entry.path());
            }
        }
    }

    #[test]
    fn adopts_readonly_files_and_relative_external_links() {
        let directory = tempfile::tempdir().unwrap();
        if !can_create_links(directory.path()) {
            return;
        }
        let mut sources = sources(directory.path());
        let external = directory.path().join("external-auth");
        fs::write(&external, "auth").unwrap();
        fs::remove_file(&sources[1].path).unwrap();
        filesystem::create_symlink(Path::new("../external-auth"), &sources[1].path).unwrap();
        let mut permissions = fs::metadata(&sources[0].path).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&sources[0].path, permissions.clone()).unwrap();
        for source in &mut sources {
            *source =
                ActiveResource::read(&spec(&source.filename, true), source.path.clone()).unwrap();
        }

        let destination = directory.path().join("profile with spaces");
        let transaction = stage_adoption(&destination, &sources).unwrap();
        // All originals are still present before commit, even though the new
        // profile does not exist and the staged symlinks are initially dangling.
        assert!(!destination.exists());
        assert!(!sources[0].path.is_symlink());
        commit_adoption(transaction, &sources).unwrap();

        for source in &sources {
            assert!(source.path.is_symlink());
            assert_eq!(
                fs::read(&source.path).unwrap(),
                source.original.as_ref().unwrap().content
            );
            assert!(!destination.join(&source.filename).is_symlink());
        }
        assert_eq!(
            fs::metadata(destination.join("config"))
                .unwrap()
                .permissions(),
            permissions
        );
        assert_eq!(fs::read_to_string(external).unwrap(), "auth");
        assert_no_staging(directory.path());
    }

    #[test]
    fn a_later_link_failure_rolls_back_profile_and_original_relative_link() {
        let directory = tempfile::tempdir().unwrap();
        if !can_create_links(directory.path()) {
            return;
        }
        let mut sources = sources(directory.path());
        fs::write(directory.path().join("external-config"), "config").unwrap();
        fs::remove_file(&sources[0].path).unwrap();
        filesystem::create_symlink(Path::new("../external-config"), &sources[0].path).unwrap();
        sources[0] =
            ActiveResource::read(&spec(&sources[0].filename, true), sources[0].path.clone())
                .unwrap();
        let destination = directory.path().join("profile");
        let transaction = stage_adoption(&destination, &sources).unwrap();
        // Force the second link operation to conflict after the directory and
        // first link have been installed; exercise the transaction's rollback.
        fs::remove_file(&sources[1].path).unwrap();
        assert!(transaction.commit().is_err());
        assert!(!destination.exists());
        assert_eq!(
            fs::read_link(&sources[0].path).unwrap(),
            Path::new("../external-config")
        );
        assert_eq!(fs::read_to_string(&sources[0].path).unwrap(), "config");
        assert!(!sources[1].path.exists());
        assert_no_staging(directory.path());
    }

    #[test]
    fn changes_after_staging_cancel_without_overwriting_current_configuration() {
        let directory = tempfile::tempdir().unwrap();
        if !can_create_links(directory.path()) {
            return;
        }
        let sources = sources(directory.path());
        let destination = directory.path().join("profile");
        let transaction = stage_adoption(&destination, &sources).unwrap();
        fs::write(&sources[1].path, "new credentials").unwrap();
        assert!(matches!(commit_adoption(transaction, &sources),
            Err(AppError::Activation(ActivationError::ConfigurationChanged(path))) if path == sources[1].path));
        assert!(!destination.exists());
        assert!(!sources[0].path.is_symlink());
        assert!(!sources[1].path.is_symlink());
        assert_eq!(
            fs::read_to_string(&sources[1].path).unwrap(),
            "new credentials"
        );
        assert_no_staging(directory.path());
    }

    #[test]
    fn optional_resources_appearing_during_adoption_are_preserved() {
        let directory = tempfile::tempdir().unwrap();
        if !can_create_links(directory.path()) {
            return;
        }
        let mut sources = sources(directory.path());
        let optional = directory.path().join("active/optional");
        sources.push(ActiveResource::read(&spec("optional", false), optional.clone()).unwrap());
        let destination = directory.path().join("profile");
        let transaction = stage_adoption(&destination, &sources).unwrap();
        fs::write(&optional, "new optional content").unwrap();
        assert!(commit_adoption(transaction, &sources).is_err());
        assert!(!destination.exists());
        assert_eq!(
            fs::read_to_string(optional).unwrap(),
            "new optional content"
        );
        assert_no_staging(directory.path());
    }

    #[test]
    fn a_copy_failure_cleans_up_already_staged_links() {
        let directory = tempfile::tempdir().unwrap();
        if !can_create_links(directory.path()) {
            return;
        }
        let sources = sources(directory.path());
        // The first source is copied and linked successfully before this fails.
        fs::remove_file(&sources[1].path).unwrap();
        let destination = directory.path().join("profile");
        assert!(stage_adoption(&destination, &sources).is_err());
        assert!(!destination.exists());
        assert!(!sources[0].path.is_symlink());
        assert_eq!(fs::read_to_string(&sources[0].path).unwrap(), "config");
        assert_no_staging(directory.path());
    }

    #[test]
    fn retargeted_links_are_detected_even_when_contents_match() {
        let directory = tempfile::tempdir().unwrap();
        if !can_create_links(directory.path()) {
            return;
        }
        let sources = sources(directory.path());
        let destination = directory.path().join("profile");
        let transaction = stage_adoption(&destination, &sources).unwrap();
        fs::write(directory.path().join("replacement"), "auth").unwrap();
        fs::remove_file(&sources[1].path).unwrap();
        filesystem::create_symlink(Path::new("../replacement"), &sources[1].path).unwrap();
        assert!(commit_adoption(transaction, &sources).is_err());
        assert!(!destination.exists());
        assert_eq!(
            fs::read_link(&sources[1].path).unwrap(),
            Path::new("../replacement")
        );
        assert_no_staging(directory.path());
    }
}
