use std::error::Error;

use aurelin_framework::auth::{ApplicationEncrypter, EncryptionError};

// Captured with aes-gcm 0.10.3, hkdf 0.12.4 and sha2 0.10.9 before upgrading.
// Fixed nonce bytes 0..12; payload is nonce followed by ciphertext and tag.
#[test]
fn application_values_encrypted_by_previous_crypto_versions_remain_readable() -> Result<(), Box<dyn Error>> {
    let encrypter = ApplicationEncrypter::new(b"primary-application-key")?;
    let payload = "AAECAwQFBgcICQoLs0t9c1AbI9qBxD2q02N6s62StKlRGZZPUU5add916JnW";
    assert_eq!(encrypter.decrypt(payload)?, b"two-factor secret");
    assert_eq!(ApplicationEncrypter::new(b"wrong-key")?.decrypt(payload), Err(EncryptionError::InvalidPayload));
    Ok(())
}

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
