use aurelin_framework::validation::issues;
use validator::{Validate, ValidationError};

#[derive(Validate)]
struct Address {
    #[validate(email)]
    email: String,
}

#[derive(Validate)]
struct Request {
    #[validate(nested)]
    addresses: Vec<Address>,
    #[validate(length(min = 8))]
    password: String,
}

#[test]
fn nested_issues_retain_indices_codes_and_safe_rule_arguments_without_values() {
    let request = Request { addresses: vec![Address { email: "not-an-email".into() }], password: "secret".into() };
    let result = issues(&request.validate().expect_err("request is invalid"), Some("profile"));
    let json = serde_json::to_string(&result).expect("issues serialize");
    assert_eq!(result[0].field, "addresses.0.email");
    assert_eq!(result[0].code, "email");
    assert_eq!(result[1].arguments["min"], 8);
    assert_eq!(result[1].bag.as_deref(), Some("profile"));
    assert!(!json.contains("not-an-email"));
    assert!(!json.contains("secret"));
}

#[test]
fn arbitrary_custom_parameters_are_not_exposed() {
    let mut errors = validator::ValidationErrors::new();
    let mut error = ValidationError::new("custom_rule");
    error.add_param("token".into(), &"sensitive");
    errors.add("field", error);
    let issue = &issues(&errors, None)[0];
    assert_eq!(issue.code, "custom_rule");
    assert!(issue.arguments.is_empty());
    assert!(!serde_json::to_string(issue).expect("issue serializes").contains("sensitive"));
}
