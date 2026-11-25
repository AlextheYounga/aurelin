//! Structured, user-facing validation failures.
//!
//! Codes are validator rule codes; custom rules should use stable, application-owned
//! codes. Input values and arbitrary validator parameters are never copied into issues.
//! This is a data contract, not a translation or message catalogue API.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use validator::{ValidationErrors, ValidationErrorsKind};

/// A single failed rule at a concrete, dot-separated field path.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ValidationIssue {
    pub field: String,
    pub code: String,
    pub descriptor: MessageDescriptor,
    pub arguments: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bag: Option<String>,
}

/// A safe, unrendered default message. Validator-provided messages are not trusted.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDescriptor {
    Email,
    Url,
    Length,
    Range,
    Required,
    Invalid,
    /// Application-provided text explicitly approved for public display.
    Custom(String),
}

impl ValidationIssue {
    /// Replace the default with application-owned text safe for public display.
    pub fn with_safe_message(mut self, message: impl Into<String>) -> Self {
        self.descriptor = MessageDescriptor::Custom(message.into());
        self
    }

    /// Render the descriptor at the presentation boundary.
    pub fn render_message(&self) -> String {
        match &self.descriptor {
            MessageDescriptor::Custom(message) => message.clone(),
            descriptor => {
                let description = match descriptor {
                    MessageDescriptor::Email => "must be a valid email address",
                    MessageDescriptor::Url => "must be a valid URL",
                    MessageDescriptor::Length => "has an invalid length",
                    MessageDescriptor::Range => "is out of range",
                    MessageDescriptor::Required => "is required",
                    MessageDescriptor::Invalid => "is invalid",
                    MessageDescriptor::Custom(_) => unreachable!(),
                };
                format!("The {} field {}.", self.field, description)
            }
        }
    }
}

/// Convert validator errors without exposing rejected values or custom parameters.
///
/// Validator messages are ignored because custom validators can embed input.
/// Custom rule codes are part of the application's public contract.
pub fn issues(errors: &ValidationErrors, bag: Option<&str>) -> Vec<ValidationIssue> {
    let mut result = Vec::new();
    collect(errors, "", bag, &mut result);
    result.sort_by(|a, b| a.field.cmp(&b.field).then(a.code.cmp(&b.code)));
    result
}

fn collect(errors: &ValidationErrors, prefix: &str, bag: Option<&str>, result: &mut Vec<ValidationIssue>) {
    for (field, kind) in errors.errors() {
        let path = if prefix.is_empty() { field.to_string() } else { format!("{prefix}.{field}") };
        match kind {
            ValidationErrorsKind::Struct(nested) => collect(nested, &path, bag, result),
            ValidationErrorsKind::List(items) => {
                for (index, nested) in items {
                    collect(nested, &format!("{path}.{index}"), bag, result);
                }
            }
            ValidationErrorsKind::Field(errors) => {
                for error in errors {
                    let mut arguments = BTreeMap::new();
                    if matches!(error.code.as_ref(), "length" | "range") {
                        for key in ["min", "max", "equal"] {
                            if let Some(value @ Value::Number(_)) = error.params.get(key) {
                                arguments.insert(key.to_owned(), value.clone());
                            }
                        }
                    }
                    result.push(ValidationIssue {
                        field: path.clone(),
                        code: error.code.to_string(),
                        descriptor: descriptor(&error.code),
                        arguments,
                        bag: bag.map(str::to_owned),
                    });
                }
            }
        }
    }
}

fn descriptor(code: &str) -> MessageDescriptor {
    match code {
        "email" => MessageDescriptor::Email,
        "url" => MessageDescriptor::Url,
        "length" => MessageDescriptor::Length,
        "range" => MessageDescriptor::Range,
        "required" => MessageDescriptor::Required,
        _ => MessageDescriptor::Invalid,
    }
}
