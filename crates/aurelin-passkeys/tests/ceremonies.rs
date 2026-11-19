use std::{error::Error, time::Duration};

use aurelin_passkeys::{PasskeyConfig, PasskeyUser, Passkeys};
use url::Url;

#[test]
fn registration_requires_discoverable_user_verified_credentials_and_login_hides_accounts() -> Result<(), Box<dyn Error>>
{
    let passkeys = Passkeys::new(PasskeyConfig {
        relying_party_id: "localhost".into(),
        relying_party_name: "Aurelin".into(),
        allowed_origins: vec![Url::parse("http://localhost:8000")?],
        user_handle_secret: b"test-user-handle-secret".to_vec(),
        timeout: Duration::from_secs(60),
    })?;
    let registration = passkeys.start_registration(
        &PasskeyUser { id: "42".into(), username: "ada@example.com".into(), display_name: "Ada Lovelace".into() },
        &[],
    )?;

    assert_eq!(registration.options["publicKey"]["authenticatorSelection"]["residentKey"], "required");
    assert_eq!(registration.options["publicKey"]["authenticatorSelection"]["userVerification"], "required");
    let login = passkeys.start_login()?;
    assert!(login.options["publicKey"]["allowCredentials"].as_array().is_some_and(Vec::is_empty));
    assert!(login.options.get("mediation").is_none());
    Ok(())
}

#[test]
fn user_handles_are_stable_opaque_and_application_specific() -> Result<(), Box<dyn Error>> {
    let config = |secret: &[u8]| PasskeyConfig {
        relying_party_id: "localhost".into(),
        relying_party_name: "Aurelin".into(),
        allowed_origins: vec![Url::parse("http://localhost:8000").expect("static URL")],
        user_handle_secret: secret.to_vec(),
        timeout: Duration::from_secs(60),
    };
    let first = Passkeys::new(config(b"first-secret"))?;
    let second = Passkeys::new(config(b"second-secret"))?;

    assert_eq!(first.user_handle("42")?, first.user_handle("42")?);
    assert_ne!(first.user_handle("42")?, first.user_handle("43")?);
    assert_ne!(first.user_handle("42")?, second.user_handle("42")?);
    Ok(())
}
