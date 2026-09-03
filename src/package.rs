use std::collections::HashSet;
use std::io::{Cursor, Read, Write};

use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::{AppError, Result};
use crate::profile::{self, ProfileResource};
use crate::targets::{self, TargetSpec};

const MAGIC: &[u8; 8] = b"CPROF\0\0\0";
const VERSION: u16 = 2;
const HEADER_LEN: usize = MAGIC.len() + 2 + 8;
const CHECKSUM_LEN: usize = 32;
const MANIFEST_MAGIC: &[u8; 4] = b"CPMF";
const MANIFEST_VERSION: u8 = 1;
const MAX_MANIFEST_ITEMS: u64 = 1_000_000;
const MAX_STRING_LEN: usize = 1024 * 1024;

pub const DEFAULT_FILE_NAME: &str = "cprof.pkg";

#[derive(Debug, Clone)]
pub struct TargetPackage {
    pub target: String,
    pub profiles: Vec<PackageProfile>,
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

#[derive(Debug)]
struct Manifest {
    targets: Vec<ManifestTarget>,
}

#[derive(Debug)]
struct ManifestTarget {
    target: String,
    profiles: Vec<ManifestProfile>,
}

#[derive(Debug)]
struct ManifestProfile {
    name: String,
    resources: Vec<usize>,
}

struct EncodedProfiles {
    manifest: Vec<ManifestProfile>,
    payloads: Vec<(String, Vec<u8>)>,
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
        let encoded = encode_profiles(target, &package.profiles, target_index)?;
        manifest_targets.push(ManifestTarget {
            target: package.target.clone(),
            profiles: encoded.manifest,
        });
        payloads.extend(encoded.payloads);
    }

    let manifest = Manifest {
        targets: manifest_targets,
    };
    let manifest_data = encode_manifest(&manifest)?;
    frame(&encode_archive(&manifest_data, payloads)?)
}

pub fn decode(data: &[u8]) -> Result<Vec<TargetPackage>> {
    let archive = unframe(data)?;
    let mut archive = ZipArchive::new(Cursor::new(archive)).map_err(invalid_archive)?;
    let manifest = read_manifest(&mut archive)?;
    if manifest.targets.is_empty() {
        return Err(AppError::InvalidPackage(
            "Package contains no targets".to_string(),
        ));
    }

    let mut target_names = HashSet::new();
    let mut packages = Vec::with_capacity(manifest.targets.len());
    for (target_index, manifest_target) in manifest.targets.iter().enumerate() {
        let target = targets::get(&manifest_target.target)?;
        if !target_names.insert(manifest_target.target.clone()) {
            return Err(AppError::InvalidPackage(format!(
                "Duplicate target '{}'",
                manifest_target.target
            )));
        }
        let profiles = decode_profiles(target, target_index, manifest_target, &mut archive)?;
        packages.push(TargetPackage::new(manifest_target.target.clone(), profiles));
    }
    Ok(packages)
}

fn encode_profiles(
    target: &'static TargetSpec,
    profiles: &[PackageProfile],
    target_index: usize,
) -> Result<EncodedProfiles> {
    if profiles.is_empty() {
        return Err(AppError::InvalidPackage(format!(
            "Target '{}' contains no profiles",
            target.id
        )));
    }
    validate_profiles(target, profiles)?;

    let mut names = HashSet::new();
    let mut manifest_profiles = Vec::with_capacity(profiles.len());
    let mut payloads = Vec::new();
    for (profile_index, profile) in profiles.iter().enumerate() {
        if !names.insert(profile.name.as_str()) {
            return Err(AppError::InvalidPackage(format!(
                "Duplicate profile '{}'",
                profile.name
            )));
        }
        let mut resource_ids = HashSet::new();
        let mut resources = Vec::with_capacity(profile.resources.len());
        for resource in &profile.resources {
            let resource_index = resource_index(target, resource)?;
            if !resource_ids.insert(resource_index) {
                return Err(AppError::InvalidPackage(format!(
                    "Duplicate resource '{}' in profile '{}'",
                    resource.spec.key, profile.name
                )));
            }
            let path = payload_path(target_index, profile_index, resource_index);
            resources.push(resource_index);
            payloads.push((path, resource.content.clone()));
        }
        manifest_profiles.push(ManifestProfile {
            name: profile.name.clone(),
            resources,
        });
    }
    Ok(EncodedProfiles {
        manifest: manifest_profiles,
        payloads,
    })
}

fn decode_profiles<R: Read + std::io::Seek>(
    target: &'static TargetSpec,
    target_index: usize,
    manifest_target: &ManifestTarget,
    archive: &mut ZipArchive<R>,
) -> Result<Vec<PackageProfile>> {
    let mut names = HashSet::new();
    let mut profiles = Vec::with_capacity(manifest_target.profiles.len());
    for (profile_index, manifest_profile) in manifest_target.profiles.iter().enumerate() {
        profile::validate_name(&manifest_profile.name)?;
        if !names.insert(manifest_profile.name.clone()) {
            return Err(AppError::InvalidPackage(format!(
                "Duplicate profile '{}'",
                manifest_profile.name
            )));
        }

        let mut resource_ids = HashSet::new();
        let mut resources = Vec::with_capacity(manifest_profile.resources.len());
        for &resource_index in &manifest_profile.resources {
            let spec = target.resources.get(resource_index).ok_or_else(|| {
                AppError::InvalidPackage(format!(
                    "Unknown resource index {resource_index} for target '{}'",
                    target.id
                ))
            })?;
            if !resource_ids.insert(resource_index) {
                return Err(AppError::InvalidPackage(format!(
                    "Duplicate resource '{}' in profile '{}'",
                    spec.key, manifest_profile.name
                )));
            }

            let path = payload_path(target_index, profile_index, resource_index);
            let mut file = archive.by_name(&path).map_err(invalid_archive)?;
            let mut content = Vec::new();
            file.read_to_end(&mut content)?;
            (spec.validate)(&content)?;
            resources.push(ProfileResource { spec, content });
        }
        if target
            .resources
            .iter()
            .enumerate()
            .any(|(index, spec)| spec.required && !resource_ids.contains(&index))
        {
            return Err(AppError::IncompleteProfile(manifest_profile.name.clone()));
        }
        profiles.push(PackageProfile::new(
            manifest_profile.name.clone(),
            resources,
        ));
    }
    if profiles.is_empty() {
        return Err(AppError::InvalidPackage(format!(
            "Target '{}' contains no profiles",
            target.id
        )));
    }
    Ok(profiles)
}

fn validate_profiles(target: &TargetSpec, profiles: &[PackageProfile]) -> Result<()> {
    for profile in profiles {
        profile::validate_name(&profile.name)?;
        let mut keys = HashSet::new();
        for resource in &profile.resources {
            let expected = target.resource(resource.spec.key)?;
            if expected.filename != resource.spec.filename || !keys.insert(resource.spec.key) {
                return Err(AppError::InvalidPackage(format!(
                    "Invalid resource '{}' in profile '{}'",
                    resource.spec.key, profile.name
                )));
            }
            (expected.validate)(&resource.content)?;
        }
        if target
            .resources
            .iter()
            .any(|spec| spec.required && !keys.contains(spec.key))
        {
            return Err(AppError::IncompleteProfile(profile.name.clone()));
        }
    }
    Ok(())
}

fn resource_index(target: &TargetSpec, resource: &ProfileResource) -> Result<usize> {
    target
        .resources
        .iter()
        .position(|spec| spec.key == resource.spec.key)
        .ok_or_else(|| {
            AppError::InvalidPackage(format!(
                "Invalid resource '{}' for target '{}'",
                resource.spec.key, target.id
            ))
        })
}

fn payload_path(target_index: usize, profile_index: usize, resource_index: usize) -> String {
    format!("p/{target_index}/{profile_index}/{resource_index}")
}

fn encode_archive(manifest: &[u8], payloads: Vec<(String, Vec<u8>)>) -> Result<Vec<u8>> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer
        .start_file("manifest.bin", options)
        .map_err(invalid_archive)?;
    writer.write_all(manifest)?;
    for (path, content) in payloads {
        writer.start_file(path, options).map_err(invalid_archive)?;
        writer.write_all(&content)?;
    }
    writer
        .finish()
        .map(|cursor| cursor.into_inner())
        .map_err(invalid_archive)
}

fn read_manifest<R: Read + std::io::Seek>(archive: &mut ZipArchive<R>) -> Result<Manifest> {
    let mut file = archive.by_name("manifest.bin").map_err(invalid_archive)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    decode_manifest(&bytes)
}

fn encode_manifest(manifest: &Manifest) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MANIFEST_MAGIC);
    bytes.push(MANIFEST_VERSION);
    write_count(&mut bytes, manifest.targets.len())?;
    for target in &manifest.targets {
        write_string(&mut bytes, &target.target)?;
        write_count(&mut bytes, target.profiles.len())?;
        for profile in &target.profiles {
            write_string(&mut bytes, &profile.name)?;
            write_count(&mut bytes, profile.resources.len())?;
            for &resource_index in &profile.resources {
                write_count(&mut bytes, resource_index)?;
            }
        }
    }
    Ok(bytes)
}

fn decode_manifest(data: &[u8]) -> Result<Manifest> {
    let mut reader = ManifestReader::new(data);
    if reader.take(MANIFEST_MAGIC.len())? != MANIFEST_MAGIC {
        return Err(AppError::InvalidPackage(
            "Invalid package manifest magic".to_string(),
        ));
    }
    if reader.read_byte()? != MANIFEST_VERSION {
        return Err(AppError::InvalidPackage(
            "Unsupported package manifest version".to_string(),
        ));
    }

    let target_count = reader.read_count()?;
    let mut targets = Vec::with_capacity(target_count);
    for _ in 0..target_count {
        let target = reader.read_string()?;
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
        targets.push(ManifestTarget { target, profiles });
    }
    reader.finish()?;
    Ok(Manifest { targets })
}

fn write_count(bytes: &mut Vec<u8>, count: usize) -> Result<()> {
    let count = u64::try_from(count)
        .map_err(|_| AppError::InvalidPackage("Manifest is too large".to_string()))?;
    write_varint(bytes, count);
    Ok(())
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
        return Err(AppError::InvalidPackage(
            "Manifest string is too long".to_string(),
        ));
    }
    write_count(bytes, value.len())?;
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

struct ManifestReader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> ManifestReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| AppError::InvalidPackage("Manifest is truncated".to_string()))?;
        if end > self.data.len() {
            return Err(AppError::InvalidPackage(
                "Manifest is truncated".to_string(),
            ));
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
                return Err(AppError::InvalidPackage(
                    "Manifest integer is too large".to_string(),
                ));
            }
            value |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(AppError::InvalidPackage(
            "Manifest integer is too large".to_string(),
        ))
    }

    fn read_count(&mut self) -> Result<usize> {
        let count = self.read_varint()?;
        if count > MAX_MANIFEST_ITEMS {
            return Err(AppError::InvalidPackage(
                "Manifest contains too many items".to_string(),
            ));
        }
        count
            .try_into()
            .map_err(|_| AppError::InvalidPackage("Manifest is too large".to_string()))
    }

    fn read_string(&mut self) -> Result<String> {
        let length = self
            .read_varint()?
            .try_into()
            .map_err(|_| AppError::InvalidPackage("Manifest string is too long".to_string()))?;
        if length > MAX_STRING_LEN {
            return Err(AppError::InvalidPackage(
                "Manifest string is too long".to_string(),
            ));
        }
        String::from_utf8(self.take(length)?.to_vec())
            .map_err(|_| AppError::InvalidPackage("Manifest contains invalid UTF-8".to_string()))
    }

    fn finish(&self) -> Result<()> {
        if self.offset != self.data.len() {
            return Err(AppError::InvalidPackage(
                "Manifest contains trailing data".to_string(),
            ));
        }
        Ok(())
    }
}

fn frame(archive: &[u8]) -> Result<Vec<u8>> {
    let archive_len = u64::try_from(archive.len())
        .map_err(|_| AppError::InvalidPackage("Package is too large".to_string()))?;
    let mut framed = Vec::with_capacity(HEADER_LEN + archive.len() + CHECKSUM_LEN);
    framed.extend_from_slice(MAGIC);
    framed.extend_from_slice(&VERSION.to_le_bytes());
    framed.extend_from_slice(&archive_len.to_le_bytes());
    framed.extend_from_slice(archive);
    framed.extend_from_slice(&checksum(&framed));
    Ok(framed)
}

fn unframe(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < HEADER_LEN + CHECKSUM_LEN || &data[..MAGIC.len()] != MAGIC {
        return Err(AppError::InvalidPackage("Not a cprof package".to_string()));
    }
    let version_offset = MAGIC.len();
    let version = u16::from_le_bytes(data[version_offset..version_offset + 2].try_into().unwrap());
    if version != VERSION {
        return Err(AppError::InvalidPackage(format!(
            "Unsupported package version: {version}"
        )));
    }

    let length_offset = version_offset + 2;
    let payload_len =
        u64::from_le_bytes(data[length_offset..length_offset + 8].try_into().unwrap());
    let payload_len: usize = payload_len
        .try_into()
        .map_err(|_| AppError::InvalidPackage("Package is too large".to_string()))?;
    let expected_len = HEADER_LEN
        .checked_add(payload_len)
        .and_then(|length| length.checked_add(CHECKSUM_LEN))
        .ok_or_else(|| AppError::InvalidPackage("Invalid package length".to_string()))?;
    if data.len() != expected_len {
        return Err(AppError::InvalidPackage(
            "Invalid package length".to_string(),
        ));
    }

    let checksum_offset = HEADER_LEN + payload_len;
    if checksum(&data[..checksum_offset]) != data[checksum_offset..] {
        return Err(AppError::ChecksumMismatch);
    }
    Ok(data[HEADER_LEN..checksum_offset].to_vec())
}

fn checksum(data: &[u8]) -> [u8; CHECKSUM_LEN] {
    Sha256::digest(data).into()
}

fn invalid_archive(error: zip::result::ZipError) -> AppError {
    AppError::InvalidPackage(format!("Invalid package archive: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_rejects_corruption() {
        let mut package = frame(b"archive bytes").unwrap();
        package[HEADER_LEN] ^= 1;
        assert!(matches!(unframe(&package), Err(AppError::ChecksumMismatch)));
    }

    #[test]
    fn manifest_roundtrip_uses_binary_encoding() {
        let manifest = Manifest {
            targets: vec![ManifestTarget {
                target: "codex".to_string(),
                profiles: vec![ManifestProfile {
                    name: "gpt".to_string(),
                    resources: vec![0, 1],
                }],
            }],
        };
        let encoded = encode_manifest(&manifest).unwrap();
        assert_eq!(&encoded[..MANIFEST_MAGIC.len()], MANIFEST_MAGIC);
        assert!(!encoded.windows(4).any(|window| window == b"name"));
        let decoded = decode_manifest(&encoded).unwrap();
        assert_eq!(decoded.targets[0].target, "codex");
        assert_eq!(decoded.targets[0].profiles[0].resources, vec![0, 1]);
    }
}
