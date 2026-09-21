//! Profile management: stored resources, active links, and adoption.
//!
//! Shared types and name validation live here. Storage handles persisted
//! profiles; activation manages active entries; adoption imports unmanaged
//! configuration. Terminal interaction and elevation belong to the command layer.

pub mod activation;
pub mod adoption;
pub mod storage;

use std::fmt;

use serde::Serialize;

use crate::config::paths;
use crate::error::{ProfileError, Result};
use crate::targets::ResourceSpec;

#[derive(Debug, Clone)]
pub struct ProfileInfo {
    pub name: String,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProfileCounts {
    pub total: usize,
    pub incomplete: usize,
}

impl fmt::Display for ProfileCounts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} ({} incomplete)", self.total, self.incomplete)
    }
}

#[derive(Debug, Clone)]
pub struct ProfileResource {
    pub spec: ResourceSpec,
    pub content: Vec<u8>,
}

pub fn validate_name(name: &str) -> Result<()> {
    if name.contains(['*', '?']) || paths::validate_safe_component(name).is_err() {
        return Err(ProfileError::InvalidName(name.to_string()).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ProfileCounts;

    #[test]
    fn profile_counts_display_total_and_incomplete_counts() {
        assert_eq!(
            ProfileCounts {
                total: 3,
                incomplete: 1,
            }
            .to_string(),
            "3 (1 incomplete)"
        );
        assert_eq!(
            ProfileCounts {
                total: 0,
                incomplete: 0,
            }
            .to_string(),
            "0 (0 incomplete)"
        );
    }
}
