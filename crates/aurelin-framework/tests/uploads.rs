use std::{error::Error, fs, io, sync::Arc};

use aurelin_framework::{
    axum::{
        Router,
        body::Body,
        extract::State,
        http::{Request, StatusCode, header::CONTENT_TYPE},
        response::IntoResponse,
        routing::post,
    },
    axum_typed_multipart::{FieldData, FieldMetadata, TryFromMultipart, TypedMultipart},
    storage::{Disk, object_store::memory::InMemory},
    uploads::{TemporaryUploadFile, request_body_limit, store_upload},
};
use tower::ServiceExt;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(TryFromMultipart)]
#[try_from_multipart(strict)]
struct UploadForm {
    #[form_data(limit = "8B")]
    file: FieldData<TemporaryUploadFile>,
    #[form_data(limit = "4B")]
    note: String,
}

#[derive(Clone)]
struct AppState {
    disk: Disk,
}

async fn accept_upload(
    State(state): State<AppState>,
    TypedMultipart(form): TypedMultipart<UploadForm>,
) -> Result<impl IntoResponse, StatusCode> {
    let UploadForm { file, note } = form;
    drop(note);
    let stored =
        store_upload(&state.disk, Some("accepted"), file).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::CREATED, stored.path.to_string()))
}

async fn reject_after_validation(TypedMultipart(_form): TypedMultipart<UploadForm>) -> StatusCode {
    StatusCode::UNPROCESSABLE_ENTITY
}

async fn deny_before_extraction() -> StatusCode {
    StatusCode::FORBIDDEN
}

fn app(disk: Disk, request_limit: usize) -> Router {
    Router::new()
        .route("/uploads", post(accept_upload))
        .route("/validation-fails", post(reject_after_validation))
        .route("/denied", post(deny_before_extraction))
        .layer(request_body_limit(request_limit))
        .with_state(AppState { disk })
}

#[derive(Clone)]
struct Part<'a> {
    name: &'a str,
    filename: Option<&'a str>,
    content_type: Option<&'a str>,
    contents: &'a [u8],
}

fn multipart(parts: &[Part<'_>]) -> (String, Vec<u8>) {
    let boundary = "aurelin-test-boundary";
    let mut body = Vec::new();
    for part in parts {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        let disposition = match part.filename {
            Some(filename) => {
                format!("Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n", part.name, filename)
            }
            None => format!("Content-Disposition: form-data; name=\"{}\"\r\n", part.name),
        };
        body.extend_from_slice(disposition.as_bytes());
        if let Some(content_type) = part.content_type {
            body.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
        }
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(part.contents);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

async fn send(app: Router, path: &str, parts: &[Part<'_>]) -> TestResult<StatusCode> {
    let (content_type, body) = multipart(parts);
    let request = Request::post(path).header(CONTENT_TYPE, content_type).body(Body::from(body))?;
    let response = app.oneshot(request).await.map_err(|error| match error {})?;
    Ok(response.status())
}

fn valid_parts<'a>(contents: &'a [u8], filename: &'a str) -> [Part<'a>; 2] {
    [
        Part { name: "file", filename: Some(filename), content_type: Some("application/untrusted"), contents },
        Part { name: "note", filename: None, content_type: None, contents: b"okay" },
    ]
}

#[tokio::test]
async fn accepted_upload_is_streamed_to_disk_with_untrusted_metadata() -> TestResult {
    let disk = Disk::object_store(Arc::new(InMemory::new()), None)?;
    let parts = valid_parts(b"contents", "../../strange file.txt");
    assert_eq!(send(app(disk.clone(), 1024), "/uploads", &parts).await?, StatusCode::CREATED);

    let objects = disk.list(Some("accepted")).await?;
    assert_eq!(objects.len(), 1);
    assert_eq!(disk.get(objects[0].path.as_str()).await?, "contents");
    assert!(!objects[0].path.as_str().contains("strange"));
    assert!(!objects[0].path.as_str().contains(".."));
    Ok(())
}

#[tokio::test]
async fn upload_metadata_is_returned_as_untrusted_and_names_do_not_collide() -> TestResult {
    let disk = Disk::object_store(Arc::new(InMemory::new()), None)?;
    let first = temporary_upload(b"one", "first.txt", "text/plain")?;
    let second = temporary_upload(b"two", "first.txt", "text/plain")?;

    let first = store_upload(&disk, Some("files"), first).await?;
    let second = store_upload(&disk, Some("files"), second).await?;

    assert_ne!(first.path, second.path);
    assert_eq!(first.untrusted_original_name.as_deref(), Some("first.txt"));
    assert_eq!(first.untrusted_content_type.as_deref(), Some("text/plain"));
    assert_eq!(first.size, 3);
    Ok(())
}

fn temporary_upload(contents: &[u8], filename: &str, content_type: &str) -> io::Result<FieldData<TemporaryUploadFile>> {
    use std::io::Write;

    let mut file = TemporaryUploadFile::new()?;
    file.write_all(contents)?;
    Ok(FieldData {
        metadata: FieldMetadata {
            file_name: Some(filename.to_owned()),
            content_type: Some(content_type.to_owned()),
            ..Default::default()
        },
        contents: file,
    })
}

#[tokio::test]
async fn empty_upload_is_stored_explicitly_as_an_empty_object() -> TestResult {
    let disk = Disk::object_store(Arc::new(InMemory::new()), None)?;
    let parts = valid_parts(b"", "empty.txt");
    assert_eq!(send(app(disk.clone(), 1024), "/uploads", &parts).await?, StatusCode::CREATED);

    let objects = disk.list(Some("accepted")).await?;
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].size, 0);
    Ok(())
}

#[tokio::test]
async fn per_file_text_field_and_total_request_limits_are_enforced() -> TestResult {
    let disk = Disk::object_store(Arc::new(InMemory::new()), None)?;

    let oversized_file = valid_parts(b"123456789", "large.bin");
    assert_eq!(send(app(disk.clone(), 1024), "/uploads", &oversized_file).await?, StatusCode::PAYLOAD_TOO_LARGE);

    let mut oversized_text = valid_parts(b"small", "small.bin");
    oversized_text[1].contents = b"large";
    assert_eq!(send(app(disk.clone(), 1024), "/uploads", &oversized_text).await?, StatusCode::PAYLOAD_TOO_LARGE);

    let valid = valid_parts(b"small", "small.bin");
    assert_eq!(send(app(disk.clone(), 64), "/uploads", &valid).await?, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(disk.list(None).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn strict_typed_forms_reject_excessive_duplicate_and_malformed_fields() -> TestResult {
    let disk = Disk::object_store(Arc::new(InMemory::new()), None)?;
    let mut excessive = valid_parts(b"small", "small.bin").to_vec();
    excessive.push(Part { name: "extra", filename: None, content_type: None, contents: b"x" });
    assert_eq!(send(app(disk.clone(), 2048), "/uploads", &excessive).await?, StatusCode::BAD_REQUEST);

    let duplicate = [
        Part { name: "file", filename: Some("one"), content_type: None, contents: b"one" },
        Part { name: "file", filename: Some("two"), content_type: None, contents: b"two" },
        Part { name: "note", filename: None, content_type: None, contents: b"okay" },
    ];
    assert_eq!(send(app(disk.clone(), 2048), "/uploads", &duplicate).await?, StatusCode::BAD_REQUEST);

    let request = Request::post("/uploads")
        .header(CONTENT_TYPE, "multipart/form-data; boundary=missing")
        .body(Body::from("not multipart"))?;
    let response = app(disk.clone(), 2048).oneshot(request).await.map_err(|error| match error {})?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let mut invalid_filename =
        b"--aurelin-invalid\r\nContent-Disposition: form-data; name=\"file\"; filename=\"".to_vec();
    invalid_filename.push(0xff);
    invalid_filename.extend_from_slice(b"\"\r\n\r\nfile\r\n--aurelin-invalid--\r\n");
    let request = Request::post("/uploads")
        .header(CONTENT_TYPE, "multipart/form-data; boundary=aurelin-invalid")
        .body(Body::from(invalid_filename))?;
    let response = app(disk.clone(), 2048).oneshot(request).await.map_err(|error| match error {})?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(disk.list(None).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn denied_or_validation_failed_requests_do_not_persist_uploads() -> TestResult {
    let disk = Disk::object_store(Arc::new(InMemory::new()), None)?;
    let parts = valid_parts(b"contents", "file.txt");

    assert_eq!(send(app(disk.clone(), 1024), "/denied", &parts).await?, StatusCode::FORBIDDEN);
    assert_eq!(send(app(disk.clone(), 1024), "/validation-fails", &parts).await?, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(disk.list(None).await?.is_empty());
    Ok(())
}

#[tokio::test]
async fn failed_storage_write_does_not_return_success() -> TestResult {
    let root = tempfile::tempdir()?;
    let root_path = root.path().to_owned();
    let disk = Disk::local(&root_path)?;
    fs::remove_dir(&root_path)?;
    fs::write(&root_path, "not a directory")?;
    let parts = valid_parts(b"contents", "file.txt");

    assert_eq!(send(app(disk, 1024), "/uploads", &parts).await?, StatusCode::INTERNAL_SERVER_ERROR);
    Ok(())
}
