use std::ffi::OsString;
use std::iter::repeat_n;
use std::path::PathBuf;

use crate::error::{ElevationError, Result};

pub(super) const ELEVATED_HOME_ARG: &str = "--elevated-home";
pub(super) const ELEVATED_CONTEXT_ARG: &str = "--elevated-context";

#[derive(Debug)]
pub(crate) struct ContextReference {
    pub(crate) path: PathBuf,
    pub(crate) digest: String,
}

/// Called only immediately after the private home prefix, never on public args.
pub(super) fn split_context_prefix(
    args: Vec<OsString>,
) -> Result<(Option<ContextReference>, Vec<OsString>)> {
    if !args.first().is_some_and(|arg| arg == ELEVATED_CONTEXT_ARG) {
        return Ok((None, args));
    }
    let mut args = args.into_iter().skip(1);
    let path = args
        .next()
        .ok_or_else(|| ElevationError::InvalidContext("missing context path".into()))?;
    let digest = args
        .next()
        .and_then(|arg| arg.into_string().ok())
        .filter(|digest| digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| {
            ElevationError::InvalidContext("missing or invalid context checksum".into())
        })?;
    Ok((
        Some(ContextReference {
            path: path.into(),
            digest,
        }),
        args.collect(),
    ))
}

/// Only consume the private prefix prepended by run_as_admin. Later arguments
/// may legitimately contain the same text as an editor argument or profile name.
pub(super) fn split_elevation_prefix(
    args: Vec<OsString>,
) -> Result<(Option<PathBuf>, Vec<OsString>)> {
    if !args.first().is_some_and(|arg| arg == ELEVATED_HOME_ARG) {
        return Ok((None, args));
    }
    let mut args = args.into_iter().skip(1);
    let home = args.next().ok_or(ElevationError::MissingHome)?;
    Ok((Some(home.into()), args.collect()))
}

pub(super) fn quote_arg(value: &OsString) -> String {
    let value = value.to_string_lossy();
    if !value.is_empty() && !value.chars().any(|ch| ch.is_whitespace() || ch == '"') {
        return value.into_owned();
    }

    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    let mut backslashes = 0;
    for character in value.chars() {
        match character {
            '\\' => backslashes += 1,
            '"' => {
                quoted.extend(repeat_n('\\', backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.extend(repeat_n('\\', backslashes));
                quoted.push(character);
                backslashes = 0;
            }
        }
    }
    quoted.extend(repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests;
