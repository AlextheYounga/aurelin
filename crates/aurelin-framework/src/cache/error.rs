use std::time::Duration;

use fred::error::Error as FredError;

/// Failures returned by cache stores and typed cache operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A value could not be encoded as the cache's JSON representation.
    #[error("encode cache value for key [{key}]: {source}")]
    Encode {
        /// Cache key whose value could not be encoded.
        key: String,
        /// JSON encoding failure.
        source: serde_json::Error,
    },
    /// Stored data did not decode as the requested Rust type.
    #[error("decode cache value for key [{key}]: {source}")]
    Decode {
        /// Cache key containing invalid or mismatched data.
        key: String,
        /// JSON decoding failure.
        source: serde_json::Error,
    },
    /// JSON null cannot be distinguished from Laravel's missing-value result.
    #[error("cache value for key [{0}] cannot be null")]
    NullValue(String),
    /// An integer mutation encountered a value that was not a signed integer.
    #[error("cache value for key [{key}] is not a signed integer")]
    NotInteger {
        /// Cache key containing the incompatible value.
        key: String,
    },
    /// An integer mutation exceeded the range of an `i64`.
    #[error("integer cache mutation overflowed for key [{0}]")]
    IntegerOverflow(String),
    /// Fred could not configure, connect to, or communicate with Redis.
    #[error("redis cache operation failed: {0}")]
    Redis(#[from] FredError),
    /// A blocking lock acquisition reached its deadline.
    #[error("timed out after {0:?} while acquiring cache lock")]
    LockTimeout(Duration),
}

/// Failure from either the cache or a `remember` value resolver.
#[derive(Debug, thiserror::Error)]
pub enum RememberError<E> {
    /// Reading or writing the cache failed.
    #[error(transparent)]
    Cache(#[from] Error),
    /// The application value resolver failed.
    #[error("cache value resolver failed: {0}")]
    Resolve(E),
}
