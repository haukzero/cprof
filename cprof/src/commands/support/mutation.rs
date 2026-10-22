//! Command mutation boundaries. Only this module maps business scopes to file locks.
//!
//! Hold a session from precondition checks through commit or recovery. `run`
//! consumes it, so an elevation retry starts after every lock has been released.

use std::path::PathBuf;

use crate::config;
use crate::error::Result;
use crate::filesystem::WriteLocks;
use crate::targets::TargetSpec;

pub(crate) fn target<T>(target: &TargetSpec, operation: impl FnOnce() -> Result<T>) -> Result<T> {
    MutationSession::targets([target.id.as_str()])?.run(operation)
}

pub(crate) struct MutationSession {
    locks: WriteLocks,
}

impl MutationSession {
    pub(crate) fn targets<'a>(ids: impl IntoIterator<Item = &'a str>) -> Result<Self> {
        Ok(Self {
            locks: WriteLocks::acquire(target_scopes(ids)?)?,
        })
    }

    pub(crate) fn configuration() -> Result<Self> {
        Ok(Self {
            locks: WriteLocks::acquire([config::extra_target_file()?])?,
        })
    }

    /// Configuration changes first lock definitions, then resolve their targets.
    pub(crate) fn include_targets<'a>(
        &mut self,
        ids: impl IntoIterator<Item = &'a str>,
    ) -> Result<()> {
        self.locks.acquire_more(target_scopes(ids)?)
    }

    pub(crate) fn run<T>(self, operation: impl FnOnce() -> Result<T>) -> Result<T> {
        operation()
    }
}

fn target_scopes<'a>(ids: impl IntoIterator<Item = &'a str>) -> Result<Vec<PathBuf>> {
    ids.into_iter().map(config::target_profiles_dir).collect()
}

#[cfg(test)]
mod tests {
    use crate::error::ProfileError;

    use super::*;

    #[test]
    fn session_releases_locks_before_returning_success_or_failure() {
        let directory = tempfile::tempdir().unwrap();
        let scope = directory.path().join("target");
        for fail in [false, true] {
            let session = MutationSession {
                locks: WriteLocks::acquire([scope.clone()]).unwrap(),
            };
            let result = session.run(|| {
                assert!(WriteLocks::acquire([scope.clone()]).is_err());
                if fail {
                    return Err(ProfileError::NotFound("test".into()).into());
                }
                Ok(())
            });
            assert_eq!(result.is_err(), fail);
            let _retry = WriteLocks::acquire([scope.clone()]).unwrap();
        }
    }
}
