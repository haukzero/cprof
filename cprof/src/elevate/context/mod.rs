//! One-shot transport for opaque command decisions. No profile contents are stored.

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempPath;
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, SECURITY_IDENTIFICATION,
};

use crate::error::{ElevationError, Result};

use super::arguments::ContextReference;

const VERSION: u32 = 1;
const MAX_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u32,
    home: Vec<u8>,
    args: Vec<Vec<u8>>,
    context: Value,
}

/// Keep the read handle ahead of TempPath so it closes before unlinking on Windows.
pub(crate) struct ContextFile {
    _file: File,
    path: TempPath,
    digest: String,
}

impl ContextFile {
    pub(crate) fn new(home: &Path, args: &[OsString], context: &Value) -> Result<Self> {
        let envelope = Envelope {
            version: VERSION,
            home: home.as_os_str().as_encoded_bytes().to_vec(),
            args: argument_bytes(args),
            context: context.clone(),
        };
        let bytes = serde_json::to_vec(&envelope).map_err(invalid)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(invalid("context is too large"));
        }
        let mut temporary = tempfile::Builder::new()
            .prefix("cprof-retry-")
            .suffix(".json")
            .tempfile()
            .map_err(invalid)?;
        temporary.write_all(&bytes).map_err(invalid)?;
        temporary.flush().map_err(invalid)?;
        let digest = checksum(&bytes);
        let path = temporary.into_temp_path();
        // Deny writes and deletion until the child exits. Verify the bytes again
        // to cover the small gap while replacing the original writable handle.
        let mut file = open_readonly(&path)?;
        if checksum(&read_bounded(&mut file)?) != digest {
            return Err(invalid("context changed while sealing it"));
        }
        Ok(Self {
            _file: file,
            path,
            digest,
        })
    }

    pub(crate) fn reference(&self) -> ContextReference {
        ContextReference {
            path: self.path.to_path_buf(),
            digest: self.digest.clone(),
        }
    }
}

pub(super) fn read_context(
    reference: &ContextReference,
    home: &Path,
    args: &[OsString],
) -> Result<Value> {
    if !reference.path.is_absolute() {
        return Err(invalid("context path must be absolute"));
    }
    let bytes = read_bounded(&mut open_readonly(&reference.path)?)?;
    if checksum(&bytes) != reference.digest {
        return Err(invalid("context checksum does not match"));
    }
    let envelope: Envelope = serde_json::from_slice(&bytes).map_err(invalid)?;
    if envelope.version != VERSION
        || envelope.home != home.as_os_str().as_encoded_bytes()
        || envelope.args != argument_bytes(args)
    {
        return Err(invalid("context belongs to a different command or version"));
    }
    Ok(envelope.context)
}

fn argument_bytes(args: &[OsString]) -> Vec<Vec<u8>> {
    args.iter()
        .map(|arg| arg.as_encoded_bytes().to_vec())
        .collect()
}

fn checksum(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn open_readonly(path: &Path) -> Result<File> {
    if std::fs::symlink_metadata(path)
        .map_err(invalid)?
        .file_type()
        .is_symlink()
    {
        return Err(invalid("context must be a regular file"));
    }
    // Never follow a substituted link or allow named-pipe impersonation.
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .security_qos_flags(SECURITY_IDENTIFICATION)
        .open(path)
        .map_err(invalid)?;
    if !file.metadata().map_err(invalid)?.is_file() {
        return Err(invalid("context must be a regular file"));
    }
    Ok(file)
}

fn read_bounded(file: &mut File) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(invalid)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(invalid("context is too large"));
    }
    Ok(bytes)
}

fn invalid(error: impl std::fmt::Display) -> crate::error::AppError {
    ElevationError::InvalidContext(error.to_string()).into()
}

#[cfg(test)]
mod tests;
