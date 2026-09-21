use std::io;
use std::path::Path;

use super::{AppError, Result};

impl AppError {
    pub(crate) fn io(path: &Path, source: io::Error) -> Self {
        Self::IoPath {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn is_io_kind(&self, kind: io::ErrorKind) -> bool {
        self.io_source().is_some_and(|source| source.kind() == kind)
    }

    pub(crate) fn io_source(&self) -> Option<&io::Error> {
        // Only direct I/O failures are retryable. Unwrapping transaction or
        // validation errors could overwrite unrecovered or unvalidated data.
        match self {
            Self::Io(source) | Self::IoPath { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub(crate) trait IoContext<T> {
    fn with_path(self, path: &Path) -> Result<T>;
}

impl<T> IoContext<T> for io::Result<T> {
    fn with_path(self, path: &Path) -> Result<T> {
        self.map_err(|source| AppError::io(path, source))
    }
}
