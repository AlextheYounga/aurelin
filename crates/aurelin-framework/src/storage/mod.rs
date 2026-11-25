//! Filesystem disks backed by [`object_store`].
//!
//! A disk is explicitly constructed by the application. Aurelin does not keep a
//! global disk registry or infer provider credentials.

mod disk;
mod error;
mod path;

pub use disk::{Disk, FileMetadata, ReadStream};
pub use error::{StorageError, StorageResult};
pub use path::StoragePath;

pub use object_store;
