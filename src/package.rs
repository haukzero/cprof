use std::io::Write;

use sha2::{Digest, Sha256};

use crate::error::{AppError, Result};

/// Package format magic bytes
pub const MAGIC: &[u8; 4] = b"CPKG";

/// Package format version
pub const VERSION: u8 = 1;

/// Minimum package size: magic(4) + version(1) + count(4) + checksum(32)
pub const MIN_SIZE: usize = 41;

/// A parsed package entry
#[derive(Debug, Clone)]
pub struct PackageEntry {
    pub name: String,
    pub content: String,
}

/// Encode profiles into the binary package format
pub fn encode(entries: &[PackageEntry]) -> Result<Vec<u8>> {
    let mut buffer: Vec<u8> = Vec::new();

    // Magic bytes
    buffer.write_all(MAGIC)?;

    // Version
    buffer.write_all(&[VERSION])?;

    // Profile count (u32 LE)
    let count = entries.len() as u32;
    buffer.write_all(&count.to_le_bytes())?;

    // Each profile
    for entry in entries {
        // Name length (u32 LE)
        let name_bytes = entry.name.as_bytes();
        let name_len = name_bytes.len() as u32;
        buffer.write_all(&name_len.to_le_bytes())?;

        // Name
        buffer.write_all(name_bytes)?;

        // Content length (u32 LE)
        let content_bytes = entry.content.as_bytes();
        let content_len = content_bytes.len() as u32;
        buffer.write_all(&content_len.to_le_bytes())?;

        // Content
        buffer.write_all(content_bytes)?;
    }

    // SHA-256 checksum of all preceding bytes
    let mut hasher = Sha256::new();
    hasher.update(&buffer);
    let checksum = hasher.finalize();
    buffer.write_all(&checksum)?;

    Ok(buffer)
}

/// Decode a binary package into entries
pub fn decode(data: &[u8]) -> Result<Vec<PackageEntry>> {
    // Minimum size check
    if data.len() < MIN_SIZE {
        return Err(AppError::InvalidPackage("File too small".to_string()));
    }

    // Verify magic
    if &data[0..4] != MAGIC {
        return Err(AppError::InvalidPackage(
            "Invalid magic bytes - not a cprof package".to_string(),
        ));
    }

    // Verify version
    if data[4] != VERSION {
        return Err(AppError::InvalidPackage(format!(
            "Unsupported package version: {}",
            data[4]
        )));
    }

    // Verify checksum
    let content_len = data.len() - 32;
    let content = &data[..content_len];
    let stored_checksum = &data[content_len..];

    let mut hasher = Sha256::new();
    hasher.update(content);
    let computed = hasher.finalize();

    if computed.as_slice() != stored_checksum {
        return Err(AppError::ChecksumMismatch);
    }

    // Parse profile count
    let count = u32::from_le_bytes(data[5..9].try_into().unwrap()) as usize;
    let mut offset = 9;
    let mut entries = Vec::with_capacity(count);

    for _ in 0..count {
        // Read name length
        if offset + 4 > content_len {
            return Err(AppError::InvalidPackage("Truncated data".to_string()));
        }
        let name_len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;

        // Read name
        if offset + name_len > content_len {
            return Err(AppError::InvalidPackage("Truncated name".to_string()));
        }
        let name = String::from_utf8(data[offset..offset + name_len].to_vec())
            .map_err(|_| AppError::InvalidPackage("Invalid UTF-8 in profile name".to_string()))?;
        offset += name_len;

        // Read content length
        if offset + 4 > content_len {
            return Err(AppError::InvalidPackage("Truncated data".to_string()));
        }
        let content_len_field =
            u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;

        // Read content
        if offset + content_len_field > content_len {
            return Err(AppError::InvalidPackage("Truncated content".to_string()));
        }
        let profile_content = String::from_utf8(data[offset..offset + content_len_field].to_vec())
            .map_err(|_| {
                AppError::InvalidPackage("Invalid UTF-8 in profile content".to_string())
            })?;
        offset += content_len_field;

        entries.push(PackageEntry {
            name,
            content: profile_content,
        });
    }

    Ok(entries)
}
