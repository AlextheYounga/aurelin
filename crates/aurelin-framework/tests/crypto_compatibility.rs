use std::error::Error;

use aurelin_framework::{auth::digest_token, scheduling::lock_key, signed_url::TemporarySignedUrl};

// Captured with hmac 0.12.1 and sha2 0.10.9 before upgrading. Round-trip
// assertions alone would not catch a change that invalidates existing URLs,
// stored token digests or cross-process scheduling lock names.
#[test]
fn existing_signatures_token_digests_and_schedule_lock_names_remain_unchanged() -> Result<(), Box<dyn Error>> {
    let signer = TemporarySignedUrl::new(b"application-key")?;
    let path = "/email/verify/1/hash?expires=2000000000";
    let signature = "9cfd00933d2ecfe318fae83888cdcc2b4d70b1b67a945fc94f0460a003c492c0";
    assert_eq!(signer.sign(path)?, signature);
    assert!(signer.verify(path, signature)?);
    assert_eq!(digest_token("compatibility-token"), "0378d73a4beb3e4cabbbc2742cb10f1d90344ea25d243ad2f5c8946e760e923d");
    assert_eq!(
        lock_key("compatibility-task"),
        "aurelin:schedule:71961e2fd9cc10a172c23dc147d0562c76f9707b00fc15f5575af71089dd4e8f"
    );
    Ok(())
}
