use crate::error::{PackageError, Result};
use crate::targets::external::{self, ExternalResourceConfig, ExternalTargetConfig};

const MAGIC: &[u8; 4] = b"CPMF";
const VERSION: u8 = 4;
const ACTIVE_TRAILER: &[u8; 4] = b"ACTV";
const MAX_ITEMS: u64 = 1_000_000;
const MAX_STRING_LEN: usize = 1024 * 1024;

#[derive(Debug)]
pub(super) struct Manifest {
    pub(super) targets: Vec<ManifestTarget>,
}

#[derive(Debug)]
pub(super) struct ManifestTarget {
    pub(super) target: String,
    pub(super) target_config: Option<ExternalTargetConfig>,
    pub(super) active_profile: Option<String>,
    pub(super) active_profile_known: bool,
    pub(super) profiles: Vec<ManifestProfile>,
}

#[derive(Debug)]
pub(super) struct ManifestProfile {
    pub(super) name: String,
    pub(super) resources: Vec<usize>,
}

pub(super) fn encode(manifest: &Manifest) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.push(VERSION);
    write_count(&mut bytes, manifest.targets.len());
    for target in &manifest.targets {
        write_string(&mut bytes, &target.target)?;
        match &target.target_config {
            None => bytes.push(0),
            Some(config) => {
                bytes.push(1);
                write_external_config(&mut bytes, config)?;
            }
        }
        write_count(&mut bytes, target.profiles.len());
        for profile in &target.profiles {
            write_string(&mut bytes, &profile.name)?;
            write_count(&mut bytes, profile.resources.len());
            for &resource_index in &profile.resources {
                write_count(&mut bytes, resource_index);
            }
        }
    }
    if manifest
        .targets
        .iter()
        .all(|target| target.active_profile_known)
    {
        bytes.extend_from_slice(ACTIVE_TRAILER);
        for target in &manifest.targets {
            write_optional_string(&mut bytes, target.active_profile.as_deref())?;
        }
    }
    Ok(bytes)
}

pub(super) fn decode(data: &[u8]) -> Result<Manifest> {
    let mut reader = Reader::new(data);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(PackageError::Invalid("Invalid package manifest magic".to_string()).into());
    }
    let version = reader.read_byte()?;
    if version != VERSION {
        return Err(
            PackageError::Invalid("Unsupported package manifest version".to_string()).into(),
        );
    }

    let target_count = reader.read_count()?;
    let mut targets = Vec::with_capacity(target_count);
    for _ in 0..target_count {
        let target = reader.read_string()?;
        let target_config = match reader.read_byte()? {
            0 => None,
            1 => Some(read_external_config(&mut reader)?),
            _ => {
                return Err(PackageError::Invalid(
                    "Manifest contains invalid target configuration flag".to_string(),
                )
                .into());
            }
        };
        let profile_count = reader.read_count()?;
        let mut profiles = Vec::with_capacity(profile_count);
        for _ in 0..profile_count {
            let name = reader.read_string()?;
            let resource_count = reader.read_count()?;
            let mut resources = Vec::with_capacity(resource_count);
            for _ in 0..resource_count {
                resources.push(reader.read_count()?);
            }
            profiles.push(ManifestProfile { name, resources });
        }
        targets.push(ManifestTarget {
            target,
            target_config,
            active_profile: None,
            active_profile_known: false,
            profiles,
        });
    }
    if reader.remaining() > 0 {
        if reader.take(ACTIVE_TRAILER.len())? != ACTIVE_TRAILER {
            return Err(
                PackageError::Invalid("Manifest contains an invalid trailer".to_string()).into(),
            );
        }
        for target in &mut targets {
            target.active_profile = reader.read_optional_string()?;
            target.active_profile_known = true;
        }
    }
    reader.finish()?;
    Ok(Manifest { targets })
}

fn write_external_config(bytes: &mut Vec<u8>, config: &ExternalTargetConfig) -> Result<()> {
    external::validate_external_config(config)
        .map_err(|error| PackageError::Invalid(format!("Invalid target configuration: {error}")))?;
    write_string(bytes, &config.name)?;
    write_optional_string(bytes, config.id.as_deref())?;
    write_count(bytes, config.resources.len());
    for resource in &config.resources {
        write_optional_string(bytes, resource.key.as_deref())?;
        write_string(bytes, &resource.filename)?;
        write_optional_string(bytes, resource.active_path.as_deref())?;
        write_optional_string(bytes, resource.absolute_active_path.as_deref())?;
        write_optional_string(bytes, resource.template.as_deref())?;
        write_optional_bool(bytes, resource.required);
    }
    Ok(())
}

fn read_external_config(reader: &mut Reader<'_>) -> Result<ExternalTargetConfig> {
    let name = reader.read_string()?;
    let id = reader.read_optional_string()?;
    let resource_count = reader.read_count()?;
    let mut resources = Vec::with_capacity(resource_count);
    for _ in 0..resource_count {
        resources.push(ExternalResourceConfig {
            key: reader.read_optional_string()?,
            filename: reader.read_string()?,
            active_path: reader.read_optional_string()?,
            absolute_active_path: reader.read_optional_string()?,
            template: reader.read_optional_string()?,
            required: reader.read_optional_bool()?,
        });
    }
    let config = ExternalTargetConfig {
        name,
        id,
        resources,
    };
    external::validate_external_config(&config)
        .map_err(|error| PackageError::Invalid(format!("Invalid target configuration: {error}")))?;
    Ok(config)
}

fn write_optional_string(bytes: &mut Vec<u8>, value: Option<&str>) -> Result<()> {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            write_string(bytes, value)?;
        }
    }
    Ok(())
}

fn write_optional_bool(bytes: &mut Vec<u8>, value: Option<bool>) {
    bytes.push(match value {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    });
}

fn write_count(bytes: &mut Vec<u8>, count: usize) {
    write_varint(bytes, count as u64);
}

fn write_varint(bytes: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        bytes.push((value as u8) | 0x80);
        value >>= 7;
    }
    bytes.push(value as u8);
}

fn write_string(bytes: &mut Vec<u8>, value: &str) -> Result<()> {
    if value.len() > MAX_STRING_LEN {
        return Err(PackageError::Invalid("Manifest string is too long".to_string()).into());
    }
    write_count(bytes, value.len());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| PackageError::Invalid("Manifest is truncated".to_string()))?;
        if end > self.data.len() {
            return Err(PackageError::Invalid("Manifest is truncated".to_string()).into());
        }
        let bytes = &self.data[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn read_byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn read_varint(&mut self) -> Result<u64> {
        let mut value = 0u64;
        for index in 0..10 {
            let byte = self.read_byte()?;
            let shift = index * 7;
            if shift == 63 && byte > 1 {
                return Err(
                    PackageError::Invalid("Manifest integer is too large".to_string()).into(),
                );
            }
            value |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(PackageError::Invalid("Manifest integer is too large".to_string()).into())
    }

    fn read_count(&mut self) -> Result<usize> {
        let count = self.read_varint()?;
        if count > MAX_ITEMS {
            return Err(
                PackageError::Invalid("Manifest contains too many items".to_string()).into(),
            );
        }
        count
            .try_into()
            .map_err(|_| PackageError::Invalid("Manifest is too large".to_string()).into())
    }

    fn read_string(&mut self) -> Result<String> {
        let length = self
            .read_varint()?
            .try_into()
            .map_err(|_| PackageError::Invalid("Manifest string is too long".to_string()))?;
        if length > MAX_STRING_LEN {
            return Err(PackageError::Invalid("Manifest string is too long".to_string()).into());
        }
        String::from_utf8(self.take(length)?.to_vec()).map_err(|_| {
            PackageError::Invalid("Manifest contains invalid UTF-8".to_string()).into()
        })
    }

    fn read_optional_string(&mut self) -> Result<Option<String>> {
        match self.read_byte()? {
            0 => Ok(None),
            1 => Ok(Some(self.read_string()?)),
            _ => Err(PackageError::Invalid(
                "Manifest contains invalid optional string flag".to_string(),
            )
            .into()),
        }
    }

    fn read_optional_bool(&mut self) -> Result<Option<bool>> {
        match self.read_byte()? {
            0 => Ok(None),
            1 => Ok(Some(false)),
            2 => Ok(Some(true)),
            _ => Err(PackageError::Invalid(
                "Manifest contains invalid optional boolean flag".to_string(),
            )
            .into()),
        }
    }

    fn finish(&self) -> Result<()> {
        if self.offset != self.data.len() {
            return Err(
                PackageError::Invalid("Manifest contains trailing data".to_string()).into(),
            );
        }
        Ok(())
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_uses_binary_encoding() {
        let manifest = Manifest {
            targets: vec![ManifestTarget {
                target: "codex".to_string(),
                target_config: None,
                active_profile: Some("gpt".to_string()),
                active_profile_known: true,
                profiles: vec![ManifestProfile {
                    name: "gpt".to_string(),
                    resources: vec![0, 1],
                }],
            }],
        };
        let encoded = encode(&manifest).unwrap();
        assert_eq!(&encoded[..MAGIC.len()], MAGIC);
        assert!(!encoded.windows(4).any(|window| window == b"name"));
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded.targets[0].target, "codex");
        assert_eq!(decoded.targets[0].active_profile.as_deref(), Some("gpt"));
        assert_eq!(decoded.targets[0].profiles[0].resources, vec![0, 1]);
    }

    #[test]
    fn roundtrip_preserves_external_target_definition() {
        let config = ExternalTargetConfig {
            name: "demo".to_string(),
            id: Some("demo-id".to_string()),
            resources: vec![ExternalResourceConfig {
                key: None,
                filename: "settings.conf".to_string(),
                active_path: Some(".config/demo/settings.conf".to_string()),
                absolute_active_path: None,
                template: Some("default".to_string()),
                required: Some(false),
            }],
        };
        let manifest = Manifest {
            targets: vec![ManifestTarget {
                target: "demo-id".to_string(),
                target_config: Some(config.clone()),
                active_profile: None,
                active_profile_known: true,
                profiles: vec![],
            }],
        };
        let decoded = decode(&encode(&manifest).unwrap()).unwrap();
        assert_eq!(decoded.targets[0].target_config.as_ref(), Some(&config));
    }
}
