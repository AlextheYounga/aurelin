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
    pub message: String,
    pub arguments: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bag: Option<String>,
}

/// Convert validator errors without exposing rejected values or custom parameters.
///
/// Custom rule messages must themselves be safe to display; custom rule codes
/// are part of the application's public contract.
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
                    let message =
                        error.message.as_ref().map_or_else(|| default_message(&path, &error.code), ToString::to_string);
                    result.push(ValidationIssue {
                        field: path.clone(),
                        code: error.code.to_string(),
                        message,
                        arguments,
                        bag: bag.map(str::to_owned),
                    });
                }
            }
        }
    }
}

fn default_message(field: &str, code: &str) -> String {
    match code {
        "email" => format!("The {field} field must be a valid email address."),
        "url" => format!("The {field} field must be a valid URL."),
        "length" => format!("The {field} field has an invalid length."),
        "range" => format!("The {field} field is out of range."),
        "required" => format!("The {field} field is required."),
        _ => format!("The {field} field is invalid."),
    }
}
