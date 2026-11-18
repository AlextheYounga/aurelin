use hmac::{Hmac, Mac};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug)]
pub struct TemporarySignedUrl {
    key: Vec<u8>,
}

impl TemporarySignedUrl {
    pub fn new(key: &[u8]) -> Result<Self, SignedUrlError> {
        if key.is_empty() {
            return Err(SignedUrlError::EmptyKey);
        }
        Ok(Self { key: key.to_vec() })
    }

    pub fn sign(&self, path_and_query: &str) -> Result<String, SignedUrlError> {
        let mut mac = HmacSha256::new_from_slice(&self.key).map_err(|_| SignedUrlError::Key)?;
        mac.update(path_and_query.as_bytes());
        Ok(format!("{:x}", mac.finalize().into_bytes()))
    }

    pub fn verify(&self, path_and_query: &str, signature: &str) -> Result<bool, SignedUrlError> {
        let Ok(signature) = decode_hex(signature) else {
            return Ok(false);
        };
        let mut mac = HmacSha256::new_from_slice(&self.key).map_err(|_| SignedUrlError::Key)?;
        mac.update(path_and_query.as_bytes());
        Ok(mac.verify_slice(&signature).is_ok())
    }
}

#[derive(Debug, Error)]
pub enum SignedUrlError {
    #[error("signed URL key must not be empty")]
    EmptyKey,
    #[error("signed URL key could not be configured")]
    Key,
}

fn decode_hex(value: &str) -> Result<Vec<u8>, ()> {
    if !value.len().is_multiple_of(2) {
        return Err(());
    }
    (0..value.len()).step_by(2).map(|index| u8::from_str_radix(&value[index..index + 2], 16).map_err(|_| ())).collect()
}
