use std::error::Error;

use aurelin_framework::auth::{ApplicationEncrypter, EncryptionError};

#[test]
fn application_values_are_authenticated_and_only_the_application_key_can_decrypt_them() -> Result<(), Box<dyn Error>> {
    let encrypter = ApplicationEncrypter::new(b"primary-application-key")?;
    let encrypted = encrypter.encrypt(b"two-factor secret")?;

    assert_ne!(encrypted, "two-factor secret");
    assert_eq!(encrypter.decrypt(&encrypted)?, b"two-factor secret");
    assert_eq!(ApplicationEncrypter::new(b"different-key")?.decrypt(&encrypted), Err(EncryptionError::InvalidPayload));

    let mut tampered = encrypted.into_bytes();
    if let Some(last) = tampered.last_mut() {
        *last = if *last == b'A' { b'B' } else { b'A' };
    }
    let tampered = String::from_utf8(tampered)?;
    assert_eq!(encrypter.decrypt(&tampered), Err(EncryptionError::InvalidPayload));
    Ok(())
}
