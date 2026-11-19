use std::error::Error;

use aurelin_framework::{
    auth::CompromisedPasswordChecker,
    axum::{
        Router,
        extract::Path,
        http::{HeaderMap, StatusCode},
        routing::get,
        serve,
    },
};
use tokio::net::TcpListener;

#[tokio::test]
async fn hibp_range_queries_send_only_a_hash_prefix_and_compare_suffixes_locally() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let application = Router::new().route("/{prefix}", get(range));
    let _server = tokio::spawn(async move { serve(listener, application).await });
    let checker = CompromisedPasswordChecker::for_endpoint(format!("http://{address}"));

    assert!(checker.is_compromised("password").await?);
    assert!(!checker.is_compromised("Correct-Horse-1").await?);
    Ok(())
}

async fn range(Path(prefix): Path<String>, headers: HeaderMap) -> (StatusCode, &'static str) {
    if headers.get("add-padding").and_then(|value| value.to_str().ok()) != Some("true") {
        return (StatusCode::BAD_REQUEST, "");
    }
    if prefix == "5BAA6" {
        return (
            StatusCode::OK,
            "1E4C9B93F3F0682250B6CF8331B7EE68FD8:3861493\r\n00000000000000000000000000000000000:0",
        );
    }
    (StatusCode::OK, "00000000000000000000000000000000000:0")
}
