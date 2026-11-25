use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::{
    http::{HeaderValue, StatusCode, header, header::IntoHeaderName},
    response::{IntoResponse, Response},
};

pub mod errors;

pub fn too_many_attempts(limit: u32, retry_after: Duration) -> Response {
    let retry_after = retry_after.as_secs().max(1);
    let mut response = (StatusCode::TOO_MANY_REQUESTS, "Too Many Attempts.").into_response();
    add_rate_limit_headers(&mut response, limit, 0, Some(retry_after));
    response
}

pub fn add_rate_limit_headers(response: &mut Response, limit: u32, remaining: u32, retry_after: Option<u64>) {
    insert_number(response, "x-ratelimit-limit", u64::from(limit));
    insert_number(response, "x-ratelimit-remaining", u64::from(remaining));
    if let Some(retry_after) = retry_after {
        insert_number(response, header::RETRY_AFTER, retry_after);
        let reset = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(retry_after, |now| now.as_secs().saturating_add(retry_after));
        insert_number(response, "x-ratelimit-reset", reset);
    }
}

fn insert_number(response: &mut Response, name: impl IntoHeaderName, value: u64) {
    if let Ok(value) = HeaderValue::from_str(&value.to_string()) {
        response.headers_mut().insert(name, value);
    }
}
