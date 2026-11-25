use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hkdf::Hkdf;
use rand::Rng;
use sha2::Sha256;
use thiserror::Error;

const NONCE_LENGTH: usize = 12;
const KEY_CONTEXT: &[u8] = b"aurelin.application-encryption.v1";

#[derive(Clone)]
pub struct ApplicationEncrypter {
    cipher: Aes256Gcm,
}

impl ApplicationEncrypter {
    pub fn new(application_key: &[u8]) -> Result<Self, EncryptionError> {
        if application_key.is_empty() {
            return Err(EncryptionError::EmptyApplicationKey);
        }
        let mut key = [0_u8; 32];
        Hkdf::<Sha256>::new(None, application_key)
            .expand(KEY_CONTEXT, &mut key)
            .map_err(|_| EncryptionError::KeyDerivation)?;
        Ok(Self { cipher: Aes256Gcm::new_from_slice(&key).map_err(|_| EncryptionError::KeyDerivation)? })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<String, EncryptionError> {
        let mut nonce = [0_u8; NONCE_LENGTH];
        rand::rng().fill(&mut nonce);
        let ciphertext = self.cipher.encrypt(&Nonce::from(nonce), plaintext).map_err(|_| EncryptionError::Encrypt)?;
        let mut payload = Vec::with_capacity(nonce.len() + ciphertext.len());
        payload.extend_from_slice(&nonce);
        payload.extend_from_slice(&ciphertext);
        Ok(URL_SAFE_NO_PAD.encode(payload))
    }

    pub fn decrypt(&self, payload: &str) -> Result<Vec<u8>, EncryptionError> {
        let payload = URL_SAFE_NO_PAD.decode(payload).map_err(|_| EncryptionError::InvalidPayload)?;
        let (nonce, ciphertext) = payload.split_at_checked(NONCE_LENGTH).ok_or(EncryptionError::InvalidPayload)?;
        let nonce = Nonce::try_from(nonce).map_err(|_| EncryptionError::InvalidPayload)?;
        self.cipher.decrypt(&nonce, ciphertext).map_err(|_| EncryptionError::InvalidPayload)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EncryptionError {
    #[error("the application encryption key cannot be empty")]
    EmptyApplicationKey,
    #[error("could not derive the application encryption key")]
    KeyDerivation,
    #[error("could not encrypt the value")]
    Encrypt,
    #[error("the encrypted payload is invalid")]
    InvalidPayload,
}
