use aurelin_framework::axum::{Json, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
pub(super) struct IssueRequest {
    pub name: String,
    pub abilities: Vec<String>,
    pub expires_at: Option<i64>,
}

impl IssueRequest {
    pub fn is_valid(&self) -> bool {
        !self.name.trim().is_empty()
            && self.name.len() <= 255
            && self.abilities.len() <= 100
            && self
                .abilities
                .iter()
                .all(|ability| !ability.is_empty() && ability.len() <= 255)
    }
}

pub(super) fn invalid_request() -> (StatusCode, Json<Value>) {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"code": "invalid_request"})),
    )
}
