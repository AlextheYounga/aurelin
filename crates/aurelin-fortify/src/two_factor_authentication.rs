use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter},
    sync::{Arc, Mutex},
};

use aurelin_framework::auth::{ApplicationEncrypter, EncryptionError};
use constant_time_eq::constant_time_eq;
use rand::{Rng, distr::Alphanumeric};
use thiserror::Error;
use totp_rs::{Builder, Secret, Totp, TotpError};

const RECOVERY_CODE_COUNT: usize = 8;
const RECOVERY_CODE_SEGMENT_LENGTH: usize = 10;

#[derive(Clone)]
pub struct TwoFactorAuthentication {
    encrypter: ApplicationEncrypter,
    issuer: Arc<str>,
    last_accepted_steps: Arc<Mutex<HashMap<String, u64>>>,
}

impl Debug for TwoFactorAuthentication {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("TwoFactorAuthentication").field("issuer", &self.issuer).finish_non_exhaustive()
    }
}

impl TwoFactorAuthentication {
    pub fn new(application_key: &[u8], issuer: impl Into<Arc<str>>) -> Result<Self, TwoFactorError> {
        Ok(Self {
            encrypter: ApplicationEncrypter::new(application_key)?,
            issuer: issuer.into(),
            last_accepted_steps: Arc::default(),
        })
    }

    pub fn enable(&self) -> Result<TwoFactorSetup, TwoFactorError> {
        let secret = Secret::generate();
        let recovery_codes = generate_recovery_codes();
        Ok(TwoFactorSetup {
            encrypted_secret: self.encrypter.encrypt(secret.as_bytes())?,
            encrypted_recovery_codes: self.encrypter.encrypt(recovery_codes.join("\n").as_bytes())?,
        })
    }

    pub fn confirm(&self, encrypted_secret: &str, code: &str, timestamp: u64) -> Result<bool, TwoFactorError> {
        self.verify_code(encrypted_secret, code, timestamp)
    }

    pub fn challenge(
        &self,
        state: TwoFactorState<'_>,
        challenge: TwoFactorChallenge<'_>,
        timestamp: u64,
    ) -> Result<TwoFactorChallengeOutcome, TwoFactorError> {
        if let Some(recovery_code) = challenge.recovery_code
            && let Some(replacement) = self.replace_recovery_code(state.encrypted_recovery_codes, recovery_code)?
        {
            return Ok(TwoFactorChallengeOutcome::RecoveryCodeAccepted { encrypted_recovery_codes: replacement });
        }
        if let Some(code) = challenge.code
            && self.verify_code(state.encrypted_secret, code, timestamp)?
        {
            return Ok(TwoFactorChallengeOutcome::CodeAccepted);
        }
        Ok(TwoFactorChallengeOutcome::Invalid)
    }

    pub fn secret_key(&self, encrypted_secret: &str) -> Result<String, TwoFactorError> {
        Ok(Secret::from(self.encrypter.decrypt(encrypted_secret)?).to_base32())
    }

    pub fn recovery_codes(&self, encrypted_recovery_codes: &str) -> Result<Vec<String>, TwoFactorError> {
        let plaintext = self.encrypter.decrypt(encrypted_recovery_codes)?;
        let plaintext = String::from_utf8(plaintext).map_err(|_| TwoFactorError::InvalidStoredValue)?;
        Ok(plaintext.lines().map(str::to_owned).collect())
    }

    pub fn qr_code(&self, encrypted_secret: &str, account_name: &str) -> Result<String, TwoFactorError> {
        let totp = self.totp(encrypted_secret, Some(account_name))?;
        Ok(totp.to_qr_base64()?)
    }

    pub fn regenerate_recovery_codes(&self) -> Result<String, TwoFactorError> {
        Ok(self.encrypter.encrypt(generate_recovery_codes().join("\n").as_bytes())?)
    }

    fn verify_code(&self, encrypted_secret: &str, code: &str, timestamp: u64) -> Result<bool, TwoFactorError> {
        if code.is_empty() {
            return Ok(false);
        }
        let Some(step) = self.totp(encrypted_secret, None)?.check(code, timestamp) else {
            return Ok(false);
        };
        let mut accepted = self.last_accepted_steps.lock().map_err(|_| TwoFactorError::ReplayState)?;
        if accepted.get(encrypted_secret).is_some_and(|previous| step <= *previous) {
            return Ok(false);
        }
        accepted.insert(encrypted_secret.to_owned(), step);
        Ok(true)
    }

    fn replace_recovery_code(
        &self,
        encrypted_recovery_codes: &str,
        candidate: &str,
    ) -> Result<Option<String>, TwoFactorError> {
        if candidate.is_empty() {
            return Ok(None);
        }
        let mut recovery_codes = self.recovery_codes(encrypted_recovery_codes)?;
        let Some(index) = recovery_codes
            .iter()
            .position(|code| code.len() == candidate.len() && constant_time_eq(code.as_bytes(), candidate.as_bytes()))
        else {
            return Ok(None);
        };
        recovery_codes[index] = generate_recovery_code();
        Ok(Some(self.encrypter.encrypt(recovery_codes.join("\n").as_bytes())?))
    }

    fn totp(&self, encrypted_secret: &str, account_name: Option<&str>) -> Result<Totp, TwoFactorError> {
        let secret = self.encrypter.decrypt(encrypted_secret)?;
        let mut builder = Builder::new().with_secret(secret).with_issuer(Some(self.issuer.as_ref()));
        if let Some(account_name) = account_name {
            builder = builder.with_account_name(account_name);
        }
        Ok(builder.build()?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TwoFactorSetup {
    pub encrypted_secret: String,
    pub encrypted_recovery_codes: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TwoFactorChallenge<'a> {
    pub code: Option<&'a str>,
    pub recovery_code: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TwoFactorState<'a> {
    pub encrypted_secret: &'a str,
    pub encrypted_recovery_codes: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TwoFactorChallengeOutcome {
    CodeAccepted,
    RecoveryCodeAccepted { encrypted_recovery_codes: String },
    Invalid,
}

#[derive(Debug, Error)]
pub enum TwoFactorError {
    #[error(transparent)]
    Encryption(#[from] EncryptionError),
    #[error(transparent)]
    Totp(#[from] TotpError),
    #[error("stored two-factor authentication data is invalid")]
    InvalidStoredValue,
    #[error("two-factor replay state is unavailable")]
    ReplayState,
}

fn generate_recovery_codes() -> Vec<String> {
    (0..RECOVERY_CODE_COUNT).map(|_| generate_recovery_code()).collect()
}

fn generate_recovery_code() -> String {
    let mut rng = rand::rng();
    let first: String =
        (&mut rng).sample_iter(Alphanumeric).take(RECOVERY_CODE_SEGMENT_LENGTH).map(char::from).collect();
    let second: String =
        (&mut rng).sample_iter(Alphanumeric).take(RECOVERY_CODE_SEGMENT_LENGTH).map(char::from).collect();
    format!("{first}-{second}")
}
