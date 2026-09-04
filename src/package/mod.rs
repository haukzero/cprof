mod archive;
mod manifest;
mod profiles;

use std::collections::HashSet;

use crate::error::{AppError, Result};
use crate::profile::ProfileResource;
use crate::targets::{self, ExternalTargetConfig};

pub(crate) const DEFAULT_FILE_NAME: &str = "cprof.pkg";

#[derive(Debug, Clone)]
pub struct TargetPackage {
    pub target: String,
    pub profiles: Vec<PackageProfile>,
    pub(crate) target_config: Option<ExternalTargetConfig>,
}

#[derive(Debug, Clone)]
pub struct PackageProfile {
    pub name: String,
    pub resources: Vec<ProfileResource>,
}

impl TargetPackage {
    pub fn new(target: impl Into<String>, profiles: Vec<PackageProfile>) -> Self {
        Self {
            target: target.into(),
            profiles,
            target_config: None,
        }
    }
}

impl PackageProfile {
    pub fn new(name: impl Into<String>, resources: Vec<ProfileResource>) -> Self {
        Self {
            name: name.into(),
            resources,
        }
    }
}

pub fn encode(packages: &[TargetPackage]) -> Result<Vec<u8>> {
    if packages.is_empty() {
        return Err(AppError::InvalidPackage("No profiles to pack".to_string()));
    }

    let mut target_names = HashSet::new();
    let mut manifest_targets = Vec::with_capacity(packages.len());
    let mut payloads = Vec::new();
    for (target_index, package) in packages.iter().enumerate() {
        let target = targets::get(&package.target)?;
        if !target_names.insert(package.target.as_str()) {
            return Err(AppError::InvalidPackage(format!(
                "Duplicate target '{}'",
                package.target
            )));
        }

        let encoded = profiles::encode(target, &package.profiles, target_index)?;
        let target_config = if targets::is_builtin(target) {
            None
        } else {
            let config = targets::external_config_for(target)?.ok_or_else(|| {
                AppError::InvalidPackage(format!(
                    "Missing configuration for external target '{}'",
                    target.id
                ))
            })?;
            Some(config.compact())
        };
        manifest_targets.push(manifest::ManifestTarget {
            target: package.target.clone(),
            target_config,
            profiles: encoded.manifest,
        });
        payloads.extend(encoded.payloads);
    }

    let manifest = manifest::Manifest {
        targets: manifest_targets,
    };
    archive::encode(&manifest::encode(&manifest)?, payloads)
}

pub fn decode(data: &[u8]) -> Result<Vec<TargetPackage>> {
    let mut archive = archive::open(data)?;
    let manifest = manifest::decode(&archive::read_manifest(&mut archive)?)?;
    if manifest.targets.is_empty() {
        return Err(AppError::InvalidPackage(
            "Package contains no targets".to_string(),
        ));
    }

    let mut target_names = HashSet::new();
    let mut packages = Vec::with_capacity(manifest.targets.len());
    for (target_index, manifest_target) in manifest.targets.iter().enumerate() {
        let target = if let Some(config) = &manifest_target.target_config {
            let target = targets::spec_from_external_config(config)?;
            if target.id != manifest_target.target {
                return Err(AppError::InvalidPackage(format!(
                    "Target configuration id '{}' does not match target '{}'",
                    target.id, manifest_target.target
                )));
            }
            Box::leak(Box::new(target))
        } else {
            targets::get(&manifest_target.target)?
        };
        if !target_names.insert(manifest_target.target.clone()) {
            return Err(AppError::InvalidPackage(format!(
                "Duplicate target '{}'",
                manifest_target.target
            )));
        }
        let profiles = profiles::decode(target, target_index, manifest_target, &mut archive)?;
        packages.push(TargetPackage {
            target: manifest_target.target.clone(),
            profiles,
            target_config: manifest_target.target_config.clone(),
        });
    }
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::targets::ExternalResourceConfig;

    #[test]
    fn decode_uses_embedded_external_target_definition() {
        let config = ExternalTargetConfig {
            name: "demo".to_string(),
            id: None,
            resources: vec![ExternalResourceConfig {
                key: None,
                filename: "settings.conf".to_string(),
                active_path: ".config/demo/settings.conf".to_string(),
                template: None,
                required: Some(false),
            }],
        };
        let manifest = manifest::Manifest {
            targets: vec![manifest::ManifestTarget {
                target: "demo".to_string(),
                target_config: Some(config.clone()),
                profiles: vec![manifest::ManifestProfile {
                    name: "empty".to_string(),
                    resources: vec![],
                }],
            }],
        };
        let package = archive::encode(&manifest::encode(&manifest).unwrap(), vec![]).unwrap();
        let decoded = decode(&package).unwrap();
        assert_eq!(decoded[0].target, "demo");
        assert_eq!(decoded[0].profiles[0].name, "empty");
        assert_eq!(decoded[0].target_config.as_ref(), Some(&config));
    }
}
