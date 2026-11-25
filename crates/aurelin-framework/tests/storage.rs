use std::{error::Error, fs, io, sync::Arc};

#[cfg(unix)]
use std::os::unix::fs::symlink;

use aurelin_framework::{
    axum::body::Bytes,
    storage::{Disk, StorageError, object_store::memory::InMemory},
};
use futures_util::{TryStreamExt, stream};
use tempfile::tempdir;
use url::Url;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

async fn verify_disk_contract(disk: Disk) -> TestResult {
    assert!(disk.missing("documents/report.txt").await?);

    disk.put("documents/report.txt", Bytes::from_static(b"first")).await?;
    assert!(disk.exists("documents/report.txt").await?);
    assert_eq!(disk.get("documents/report.txt").await?, "first");

    disk.put("documents/report.txt", Bytes::from_static(b"replacement")).await?;
    assert_eq!(disk.get("documents/report.txt").await?, "replacement");

    let source =
        stream::iter([Ok::<_, io::Error>(Bytes::from_static(b"streamed ")), Ok(Bytes::from_static(b"contents"))]);
    assert_eq!(disk.put_stream("stream.bin", source).await?, 17);
    let chunks = disk.read_stream("stream.bin").await?.try_collect::<Vec<_>>().await?;
    assert_eq!(chunks.concat(), b"streamed contents");

    disk.copy("stream.bin", "copy.bin").await?;
    assert_eq!(disk.get("copy.bin").await?, "streamed contents");
    disk.move_file("copy.bin", "moved.bin").await?;
    assert!(disk.missing("copy.bin").await?);
    assert_eq!(disk.get("moved.bin").await?, "streamed contents");

    let metadata = disk.metadata("moved.bin").await?;
    assert_eq!(metadata.size, 17);
    assert_eq!(metadata.path.as_str(), "moved.bin");

    let listed = disk.list(Some("documents")).await?;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].path.as_str(), "documents/report.txt");

    disk.delete("moved.bin").await?;
    disk.delete("moved.bin").await?;
    assert!(matches!(disk.get("moved.bin").await, Err(StorageError::NotFound { .. })));
    Ok(())
}

#[tokio::test]
async fn in_memory_store_satisfies_the_shared_disk_contract() -> TestResult {
    let disk = Disk::object_store(Arc::new(InMemory::new()), None)?;
    verify_disk_contract(disk).await
}

#[tokio::test]
async fn local_store_satisfies_the_shared_disk_contract() -> TestResult {
    let directory = tempdir()?;
    verify_disk_contract(Disk::local(directory.path())?).await
}

#[tokio::test]
async fn prefixes_isolate_disks_and_clones_share_the_store() -> TestResult {
    let store = Arc::new(InMemory::new());
    let first = Disk::object_store(store.clone(), Some("first"))?;
    let second = Disk::object_store(store, Some("second"))?;

    first.put("shared.txt", Bytes::from_static(b"first disk")).await?;
    assert!(second.missing("shared.txt").await?);
    second.put("shared.txt", Bytes::from_static(b"second disk")).await?;

    let clone = first.clone();
    assert_eq!(clone.get("shared.txt").await?, "first disk");
    assert_eq!(second.get("shared.txt").await?, "second disk");
    Ok(())
}

#[tokio::test]
async fn failed_stream_does_not_publish_a_partial_object() -> TestResult {
    let disk = Disk::object_store(Arc::new(InMemory::new()), None)?;
    let source = stream::iter([Ok(Bytes::from_static(b"partial")), Err(io::Error::other("source failed"))]);

    assert!(matches!(disk.put_stream("failed.bin", source).await, Err(StorageError::InputStream(_))));
    assert!(disk.missing("failed.bin").await?);
    Ok(())
}

#[test]
fn rejects_unsafe_object_paths() -> TestResult {
    let disk = Disk::object_store(Arc::new(InMemory::new()), None)?;
    for path in [
        "../secret",
        "directory/../secret",
        "/absolute",
        "C:\\Windows\\secret",
        "C:/Windows/secret",
        "directory\\..\\secret",
        "directory//file",
        "directory/",
        "nul\0file",
        "",
    ] {
        assert!(matches!(disk.url(path), Err(StorageError::InvalidPath { .. })), "accepted {path:?}");
    }
    Ok(())
}

#[tokio::test]
async fn local_disk_rejects_lexical_escape_but_matches_laravel_symlink_behavior() -> TestResult {
    let root = tempdir()?;
    let outside = tempdir()?;
    fs::write(outside.path().join("outside.txt"), "outside")?;
    let disk = Disk::local(root.path())?;

    assert!(matches!(disk.get("../outside.txt").await, Err(StorageError::InvalidPath { .. })));

    #[cfg(unix)]
    {
        symlink(outside.path(), root.path().join("external"))?;
        assert_eq!(disk.get("external/outside.txt").await?, "outside");
    }
    Ok(())
}

#[tokio::test]
async fn local_backend_failures_are_not_reported_as_missing_objects() -> TestResult {
    let root = tempdir()?;
    let root_path = root.path().to_owned();
    let disk = Disk::local(&root_path)?;
    fs::remove_dir(&root_path)?;
    fs::write(&root_path, "not a directory")?;

    assert!(matches!(disk.exists("object.txt").await, Err(StorageError::Backend(_))));
    Ok(())
}

#[test]
fn public_urls_are_encoded_and_capabilities_are_explicit() -> TestResult {
    let disk = Disk::object_store(Arc::new(InMemory::new()), None)?
        .with_public_url(Url::parse("https://cdn.example.test/assets/")?);

    assert_eq!(disk.url("reports/a file#1.txt")?.as_str(), "https://cdn.example.test/assets/reports/a%20file%231.txt");
    assert!(matches!(disk.visibility("file.txt"), Err(StorageError::UnsupportedCapability { .. })));
    assert!(matches!(disk.temporary_url("file.txt"), Err(StorageError::UnsupportedCapability { .. })));

    let no_url = Disk::object_store(Arc::new(InMemory::new()), None)?;
    assert!(matches!(no_url.url("file.txt"), Err(StorageError::UnsupportedCapability { .. })));
    Ok(())
}
