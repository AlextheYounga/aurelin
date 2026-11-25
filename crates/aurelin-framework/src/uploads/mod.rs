//! Safe temporary multipart uploads and explicit storage.
//!
//! Applications define typed forms with [`axum_typed_multipart::TryFromMultipart`].
//! Use `#[try_from_multipart(strict)]` to reject unknown and duplicate fields,
//! `#[form_data(limit = "...")]` on every file and text field, and
//! [`request_body_limit`] on the route. File fields should use
//! `FieldData<NamedTempFile>` so request bodies are streamed to temporary files.
//! Authorization and application validation should run before [`store_upload`].

use std::{io, path::Path as FilePath};

use axum::{body::Bytes, extract::DefaultBodyLimit};
use axum_typed_multipart::FieldData;
use futures_util::stream;
use rand::{Rng, distr::Alphanumeric};
use tempfile::NamedTempFile;
use tokio::{fs::File, io::AsyncReadExt};

use crate::storage::{Disk, StorageError, StoragePath, StorageResult};

const READ_CHUNK_SIZE: usize = 64 * 1024;
const GENERATED_NAME_LENGTH: usize = 40;

/// Metadata returned after an accepted temporary upload is stored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredUpload {
    /// Generated disk-relative path. It never contains the original filename.
    pub path: StoragePath,
    /// Number of bytes written to the disk.
    pub size: u64,
    /// Client-reported filename, retained only as untrusted metadata.
    pub untrusted_original_name: Option<String>,
    /// Client-reported content type, retained only as untrusted metadata.
    pub untrusted_content_type: Option<String>,
}

/// Applies an Axum total request-body limit while multipart fields are read.
///
/// Per-field limits remain part of the typed form declaration, where they are
/// enforced while `axum-typed-multipart` consumes each field.
pub fn request_body_limit(max_bytes: usize) -> DefaultBodyLimit {
    DefaultBodyLimit::max(max_bytes)
}

/// Stores a validated temporary upload under an opaque generated name.
///
/// `directory` is selected by the application and must be a safe relative path.
/// The original filename and content type are metadata only. No extension or
/// MIME claim supplied by the client influences the stored path.
pub async fn store_upload(
    disk: &Disk,
    directory: Option<&str>,
    upload: FieldData<NamedTempFile>,
) -> StorageResult<StoredUpload> {
    let directory = directory.map(StoragePath::parse).transpose()?;
    let generated_name: String =
        rand::rng().sample_iter(Alphanumeric).take(GENERATED_NAME_LENGTH).map(char::from).collect();
    let path = match directory {
        Some(directory) => StoragePath::parse(format!("{directory}/{generated_name}"))?,
        None => StoragePath::parse(generated_name)?,
    };

    let original_name = upload.metadata.file_name;
    let content_type = upload.metadata.content_type;
    let file = open_temporary_file(upload.contents.path()).await?;
    let stream = file_stream(file);
    let size = disk.put_stream(path.as_str(), stream).await?;

    Ok(StoredUpload { path, size, untrusted_original_name: original_name, untrusted_content_type: content_type })
}

async fn open_temporary_file(path: &FilePath) -> StorageResult<File> {
    File::open(path).await.map_err(StorageError::input)
}

fn file_stream(file: File) -> impl futures_util::Stream<Item = Result<Bytes, io::Error>> + Send {
    stream::try_unfold(file, |mut file| async move {
        let mut buffer = vec![0_u8; READ_CHUNK_SIZE];
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            return Ok(None);
        }
        buffer.truncate(read);
        Ok(Some((Bytes::from(buffer), file)))
    })
}

/// Re-export of the streaming temporary-file type used in typed multipart forms.
pub use tempfile::NamedTempFile as TemporaryUploadFile;
