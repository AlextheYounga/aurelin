use std::{fmt, str::FromStr};

use object_store::path::Path;

use super::{StorageError, StorageResult};

/// A validated, non-empty path relative to a disk root.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StoragePath(Path);

impl StoragePath {
    /// Validates an application path without silently changing it.
    pub fn parse(value: impl AsRef<str>) -> StorageResult<Self> {
        let value = value.as_ref();
        validate_shape(value)?;
        let path = Path::parse(value)
            .map_err(|error| StorageError::InvalidPath { path: value.to_owned(), reason: error.to_string() })?;
        if path.is_root() {
            return Err(invalid(value, "a file path cannot be empty"));
        }
        Ok(Self(path))
    }

    pub(crate) fn object_path(&self) -> &Path {
        &self.0
    }

    pub(crate) fn from_object_path(path: Path) -> Self {
        Self(path)
    }

    /// Returns the validated relative path.
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    /// Returns the individual path segments.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.as_str().split('/')
    }
}

fn validate_shape(value: &str) -> StorageResult<()> {
    if value.starts_with('/') {
        return Err(invalid(value, "absolute paths are not allowed"));
    }
    if value.ends_with('/') {
        return Err(invalid(value, "trailing separators are not allowed"));
    }
    if value.contains('\\') {
        return Err(invalid(value, "backslash separators are not allowed"));
    }
    if value.as_bytes().get(1) == Some(&b':') && value.as_bytes().first().is_some_and(u8::is_ascii_alphabetic) {
        return Err(invalid(value, "Windows absolute paths are not allowed"));
    }
    if value.split('/').any(|segment| segment.is_empty()) {
        return Err(invalid(value, "empty path segments are not allowed"));
    }
    Ok(())
}

fn invalid(path: &str, reason: &str) -> StorageError {
    StorageError::InvalidPath { path: path.to_owned(), reason: reason.to_owned() }
}

impl AsRef<str> for StoragePath {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for StoragePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for StoragePath {
    type Err = StorageError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl TryFrom<&str> for StoragePath {
    type Error = StorageError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}
