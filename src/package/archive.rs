use std::io::{Cursor, Read, Seek, Write};

use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::{AppError, Result};

const MAGIC: &[u8; 8] = b"CPROF\0\0\0";
const VERSION: u16 = 2;
const HEADER_LEN: usize = MAGIC.len() + 2 + 8;
const CHECKSUM_LEN: usize = 32;

pub(super) fn open(data: &[u8]) -> Result<ZipArchive<Cursor<Vec<u8>>>> {
    let archive = unframe(data)?;
    ZipArchive::new(Cursor::new(archive)).map_err(invalid_archive)
}

pub(super) fn encode(manifest: &[u8], payloads: Vec<(String, Vec<u8>)>) -> Result<Vec<u8>> {
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
    let archive = writer
        .finish()
        .map(|cursor| cursor.into_inner())
        .map_err(invalid_archive)?;
    Ok(frame(&archive))
}

pub(super) fn read_manifest<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<Vec<u8>> {
    let mut file = archive.by_name("manifest.bin").map_err(invalid_archive)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn frame(archive: &[u8]) -> Vec<u8> {
    let archive_len = archive.len() as u64;
    let mut framed = Vec::with_capacity(HEADER_LEN + archive.len() + CHECKSUM_LEN);
    framed.extend_from_slice(MAGIC);
    framed.extend_from_slice(&VERSION.to_le_bytes());
    framed.extend_from_slice(&archive_len.to_le_bytes());
    framed.extend_from_slice(archive);
    framed.extend_from_slice(&checksum(&framed));
    framed
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
    use super::{HEADER_LEN, frame, unframe};
    use crate::error::AppError;

    #[test]
    fn envelope_rejects_corruption() {
        let mut package = frame(b"archive bytes");
        package[HEADER_LEN] ^= 1;
        assert!(matches!(unframe(&package), Err(AppError::ChecksumMismatch)));
    }
}
