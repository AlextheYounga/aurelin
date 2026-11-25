use std::error::Error;

use object_store::Error as ObjectStoreError;

/// Result returned by filesystem operations.
pub type StorageResult<T> = Result<T, StorageError>;

/// A filesystem failure with path and capability errors separated from backend failures.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The application supplied an unsafe or malformed relative path.
    #[error("invalid storage path '{path}': {reason}")]
    InvalidPath { path: String, reason: String },

    /// The requested object does not exist.
    #[error("object '{path}' was not found")]
    NotFound { path: String },

    /// The selected disk does not provide the requested capability.
    #[error("disk does not support {capability}")]
    UnsupportedCapability { capability: &'static str },

    /// Reading a caller-provided stream failed.
    #[error("input stream failed: {0}")]
    InputStream(#[source] Box<dyn Error + Send + Sync>),

    /// The underlying object store failed.
    #[error("storage backend failed: {0}")]
    Backend(#[source] ObjectStoreError),

    /// Local disk setup failed.
    #[error("local disk setup failed: {0}")]
    LocalSetup(#[source] ObjectStoreError),
}

impl StorageError {
    pub(crate) fn backend(error: ObjectStoreError, path: &str) -> Self {
        match error {
            ObjectStoreError::NotFound { .. } => Self::NotFound { path: path.to_owned() },
            error => Self::Backend(error),
        }
    }

    pub(crate) fn input(error: impl Error + Send + Sync + 'static) -> Self {
        Self::InputStream(Box::new(error))
    }
}
