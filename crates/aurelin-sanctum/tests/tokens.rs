use std::error::Error;

use aurelin_sanctum::{Abilities, NewToken, ParsedToken, StoredToken};

#[test]
fn generated_token_round_trips_without_exposing_secret_in_debug() -> Result<(), Box<dyn Error>> {
    let token = NewToken::generate();
    let digest = token.digest();
    assert_eq!(format!("{token:?}"), "NewToken([REDACTED])");
    let plaintext = token.into_plaintext(42);
    let parsed = ParsedToken::parse(&plaintext)?;
    assert_eq!(parsed.id(), 42);
    assert!(parsed.matches_digest(&digest));
    assert!(!format!("{parsed:?}").contains(&plaintext));
    assert!(!parsed.matches_digest(&"0".repeat(64)));
    Ok(())
}

#[test]
fn expiration_is_optional_and_deadline_is_exclusive() {
    let mut token = StoredToken { owner: 1, digest: String::new(), abilities: Abilities::default(), expires_at: None };
    assert!(!token.is_expired(100));
    token.expires_at = Some(100);
    assert!(!token.is_expired(99));
    assert!(token.is_expired(100));
}

#[test]
fn malformed_credentials_are_rejected() {
    for value in ["", "1", "|secret", "a|secret", "1|", "1|secret", "1|secret|other"] {
        assert!(ParsedToken::parse(value).is_err());
    }
}

#[test]
fn abilities_support_wildcard_exact_all_and_any_checks() {
    let limited = Abilities::new(vec!["read".into()]);
    assert!(limited.allows("read"));
    assert!(!limited.allows("write"));
    assert!(!limited.allows_all(&["read", "write"]));
    assert!(limited.allows_any(&["read", "write"]));
    assert!(!limited.allows_any(&[]));
    assert!(Abilities::default().allows_all(&["read", "write"]));
}
