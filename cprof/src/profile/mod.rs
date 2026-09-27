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
