use std::error::Error;

use aurelin_fortify::{EmailVerification, EmailVerificationRequest};

#[test]
fn verification_links_bind_the_user_email_expiration_and_signature() -> Result<(), Box<dyn Error>> {
    let verification = EmailVerification::new(b"application-key")?;
    let url = verification.url(7, "ada@example.com", 1_000)?;
    let path = url.strip_prefix("/email/verify/7/").ok_or("unexpected path")?;
    let (email_hash, query) = path.split_once("?expires=").ok_or("missing expiration")?;
    let (expires, signature) = query.split_once("&signature=").ok_or("missing signature")?;
    let expires = expires.parse::<i64>()?;

    let request = EmailVerificationRequest { user_id: 7, email: "ada@example.com", email_hash, expires, signature };
    assert!(verification.verify(request, 1_100)?);
    assert!(!verification.verify(
        EmailVerificationRequest { user_id: 8, email: "ada@example.com", email_hash, expires, signature },
        1_100,
    )?);
    assert!(!verification.verify(
        EmailVerificationRequest { user_id: 7, email: "other@example.com", email_hash, expires, signature },
        1_100,
    )?);
    assert!(!verification.verify(
        EmailVerificationRequest { user_id: 7, email: "ada@example.com", email_hash, expires, signature },
        expires + 1,
    )?);
    Ok(())
}
