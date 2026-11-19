use std::net::SocketAddr;

use crate::{PASSKEY_ATTEMPTS_PER_MINUTE, PasskeyRateLimiter};
use aurelin_framework::{auth::RateLimitOutcome, axum::response::Response};
use serde_json::Value;

use aurelin_framework::http as rate_limit;

use super::super::{AuthSession, FortifyBackend};

pub(super) struct PasskeyLimit {
    remaining: u32,
}

impl PasskeyLimit {
    pub(super) fn add_headers(&self, mut response: Response) -> Response {
        rate_limit::add_rate_limit_headers(&mut response, PASSKEY_ATTEMPTS_PER_MINUTE, self.remaining, None);
        response
    }
}

pub(super) async fn attempt<B: FortifyBackend>(
    auth: &AuthSession<B>,
    address: SocketAddr,
    credential: Option<&Value>,
) -> Result<PasskeyLimit, Response> {
    let identifier = credential
        .and_then(|credential| credential.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| auth.session.id().map(|id| id.to_string()))
        .unwrap_or_else(|| "anonymous".into());
    let key = PasskeyRateLimiter::key(&identifier, address.ip());
    match auth.backend.passkey_attempt(&key).await {
        RateLimitOutcome::Allowed { remaining } => Ok(PasskeyLimit { remaining }),
        RateLimitOutcome::Limited { retry_after } => {
            Err(rate_limit::too_many_attempts(PASSKEY_ATTEMPTS_PER_MINUTE, retry_after))
        }
    }
}
