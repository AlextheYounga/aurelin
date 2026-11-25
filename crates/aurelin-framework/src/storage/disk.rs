use std::{error::Error, fmt, io, path::Path as FilePath, pin::Pin, sync::Arc};

use axum::body::Bytes;
use chrono::{DateTime, Utc};
use futures_util::{Stream, StreamExt, TryStreamExt};
use object_store::{
    ObjectStore, ObjectStoreExt, PutPayload, WriteMultipart, local::LocalFileSystem, prefix::PrefixStore,
};
use url::Url;

use super::{StorageError, StoragePath, StorageResult};

/// A streaming object read. Chunks are produced without collecting the object in memory.
pub type ReadStream = Pin<Box<dyn Stream<Item = StorageResult<Bytes>> + Send>>;

/// Backend-independent file metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileMetadata {
    /// Disk-relative object path.
    pub path: StoragePath,
    /// Object size in bytes.
    pub size: u64,
    /// Backend-reported last modification time.
    pub last_modified: DateTime<Utc>,
}

/// A cloneable filesystem disk over a shared [`ObjectStore`].
#[derive(Clone)]
pub struct Disk {
    store: Arc<dyn ObjectStore>,
    public_url: Option<Url>,
}

impl fmt::Debug for Disk {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Disk")
            .field("store", &self.store.to_string())
            .field("public_url", &self.public_url)
            .finish()
    }
}

impl Disk {
    /// Creates a disk from an existing object store and optional store prefix.
    pub fn object_store(store: Arc<dyn ObjectStore>, prefix: Option<&str>) -> StorageResult<Self> {
        let store: Arc<dyn ObjectStore> = match prefix {
            Some(prefix) => {
                let prefix = StoragePath::parse(prefix)?;
                Arc::new(PrefixStore::new(store, prefix.object_path().clone()))
            }
            None => store,
        };
        Ok(Self { store, public_url: None })
    }

    /// Creates a local disk rooted at an explicitly configured directory.
    ///
    /// Like Laravel's Flysystem local driver, the directory is a trusted root.
    /// Direct operations follow operating-system symbolic links.
    pub fn local(root: impl AsRef<FilePath>) -> StorageResult<Self> {
        let store = LocalFileSystem::new_with_prefix(root).map_err(StorageError::LocalSetup)?;
        Ok(Self { store: Arc::new(store), public_url: None })
    }

    /// Configures a public base URL for this disk.
    ///
    /// The URL represents the disk root; object path segments are appended with
    /// URL encoding. This does not make objects public by itself.
    pub fn with_public_url(mut self, public_url: Url) -> Self {
        self.public_url = Some(public_url);
        self
    }

    /// Returns whether an object exists. Backend failures are not treated as misses.
    pub async fn exists(&self, path: impl AsRef<str>) -> StorageResult<bool> {
        let path = StoragePath::parse(path)?;
        match self.store.head(path.object_path()).await {
            Ok(_) => Ok(true),
            Err(object_store::Error::NotFound { .. }) => Ok(false),
            Err(error) => Err(StorageError::Backend(error)),
        }
    }

    /// Returns whether an object does not exist.
    pub async fn missing(&self, path: impl AsRef<str>) -> StorageResult<bool> {
        self.exists(path).await.map(|exists| !exists)
    }

    /// Reads an entire object into memory.
    pub async fn get(&self, path: impl AsRef<str>) -> StorageResult<Bytes> {
        let path = StoragePath::parse(path)?;
        let result =
            self.store.get(path.object_path()).await.map_err(|error| StorageError::backend(error, path.as_str()))?;
        result.bytes().await.map_err(|error| StorageError::backend(error, path.as_str()))
    }

    /// Opens an object as an asynchronous byte stream.
    pub async fn read_stream(&self, path: impl AsRef<str>) -> StorageResult<ReadStream> {
        let path = StoragePath::parse(path)?;
        let result =
            self.store.get(path.object_path()).await.map_err(|error| StorageError::backend(error, path.as_str()))?;
        let path = path.to_string();
        Ok(Box::pin(result.into_stream().map_err(move |error| StorageError::backend(error, &path))))
    }

    /// Atomically writes a buffered object, replacing an existing object.
    pub async fn put(&self, path: impl AsRef<str>, contents: Bytes) -> StorageResult<()> {
        let path = StoragePath::parse(path)?;
        self.store
            .put(path.object_path(), PutPayload::from(contents))
            .await
            .map_err(|error| StorageError::backend(error, path.as_str()))?;
        Ok(())
    }

    /// Streams an object through the backend multipart API, replacing an existing object.
    ///
    /// Failed streams abort the backend upload before returning where the backend permits it.
    pub async fn put_stream<S, E>(&self, path: impl AsRef<str>, stream: S) -> StorageResult<u64>
    where
        S: Stream<Item = Result<Bytes, E>> + Send,
        E: Error + Send + Sync + 'static,
    {
        let path = StoragePath::parse(path)?;
        let upload = self
            .store
            .put_multipart(path.object_path())
            .await
            .map_err(|error| StorageError::backend(error, path.as_str()))?;
        let mut writer = WriteMultipart::new(upload);
        let mut size = 0_u64;
        futures_util::pin_mut!(stream);

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => {
                    writer.abort().await.map_err(|error| StorageError::backend(error, path.as_str()))?;
                    return Err(StorageError::input(error));
                }
            };
            let chunk_size = match u64::try_from(chunk.len()) {
                Ok(chunk_size) => chunk_size,
                Err(error) => {
                    writer.abort().await.map_err(|error| StorageError::backend(error, path.as_str()))?;
                    return Err(StorageError::input(error));
                }
            };
            size = match size.checked_add(chunk_size) {
                Some(size) => size,
                None => {
                    writer.abort().await.map_err(|error| StorageError::backend(error, path.as_str()))?;
                    return Err(StorageError::input(io::Error::other("input stream length exceeds u64")));
                }
            };
            if let Err(error) = writer.wait_for_capacity(4).await {
                writer.abort().await.map_err(|abort_error| StorageError::backend(abort_error, path.as_str()))?;
                return Err(StorageError::backend(error, path.as_str()));
            }
            writer.put(chunk);
        }

        writer.finish().await.map_err(|error| StorageError::backend(error, path.as_str()))?;
        Ok(size)
    }

    /// Deletes an object. Deleting an absent object succeeds, matching Laravel's default behavior.
    pub async fn delete(&self, path: impl AsRef<str>) -> StorageResult<()> {
        let path = StoragePath::parse(path)?;
        match self.store.delete(path.object_path()).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(error) => Err(StorageError::Backend(error)),
        }
    }

    /// Copies an object, replacing the destination when supported by the backend.
    pub async fn copy(&self, source: impl AsRef<str>, destination: impl AsRef<str>) -> StorageResult<()> {
        let source = StoragePath::parse(source)?;
        let destination = StoragePath::parse(destination)?;
        self.store
            .copy(source.object_path(), destination.object_path())
            .await
            .map_err(|error| StorageError::backend(error, source.as_str()))
    }

    /// Moves an object, replacing the destination when supported by the backend.
    pub async fn move_file(&self, source: impl AsRef<str>, destination: impl AsRef<str>) -> StorageResult<()> {
        let source = StoragePath::parse(source)?;
        let destination = StoragePath::parse(destination)?;
        self.store
            .rename(source.object_path(), destination.object_path())
            .await
            .map_err(|error| StorageError::backend(error, source.as_str()))
    }

    /// Returns object size and last-modified metadata.
    pub async fn metadata(&self, path: impl AsRef<str>) -> StorageResult<FileMetadata> {
        let path = StoragePath::parse(path)?;
        let metadata =
            self.store.head(path.object_path()).await.map_err(|error| StorageError::backend(error, path.as_str()))?;
        Ok(FileMetadata { path, size: metadata.size, last_modified: metadata.last_modified })
    }

    /// Lists objects below an optional disk-relative prefix.
    pub async fn list(&self, prefix: Option<&str>) -> StorageResult<Vec<FileMetadata>> {
        let prefix = prefix.map(StoragePath::parse).transpose()?;
        self.store
            .list(prefix.as_ref().map(StoragePath::object_path))
            .map_ok(|metadata| FileMetadata {
                path: StoragePath::from_object_path(metadata.location),
                size: metadata.size,
                last_modified: metadata.last_modified,
            })
            .map_err(StorageError::Backend)
            .try_collect()
            .await
    }

    /// Generates a URL using the configured public base URL.
    pub fn url(&self, path: impl AsRef<str>) -> StorageResult<Url> {
        let path = StoragePath::parse(path)?;
        let mut url = self
            .public_url
            .clone()
            .ok_or(StorageError::UnsupportedCapability { capability: "public URL generation" })?;
        {
            let mut segments = url.path_segments_mut().map_err(|()| StorageError::UnsupportedCapability {
                capability: "public URL generation for a non-hierarchical base URL",
            })?;
            segments.pop_if_empty();
            for segment in path.segments() {
                segments.push(segment);
            }
        }
        Ok(url)
    }

    /// Reports that generic object stores do not expose portable visibility mutation.
    pub fn visibility(&self, _path: impl AsRef<str>) -> StorageResult<()> {
        Err(StorageError::UnsupportedCapability { capability: "portable object visibility" })
    }

    /// Reports that this initial generic disk does not expose provider-specific URL signing.
    pub fn temporary_url(&self, _path: impl AsRef<str>) -> StorageResult<Url> {
        Err(StorageError::UnsupportedCapability { capability: "temporary URL signing" })
    }
}
