//! Laravel-style passkey ceremonies backed by `webauthn-rs`.

use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter},
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use thiserror::Error;
use url::Url;
use uuid::Uuid;
use webauthn_rs::prelude::{
    CredentialID, DiscoverableAuthentication, DiscoverableKey, Passkey, PasskeyAuthentication, PasskeyRegistration,
    PublicKeyCredential, RegisterPublicKeyCredential, Webauthn, WebauthnBuilder, WebauthnError,
};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug)]
pub struct PasskeyConfig {
    pub relying_party_id: String,
    pub relying_party_name: String,
    pub allowed_origins: Vec<Url>,
    pub user_handle_secret: Vec<u8>,
    pub timeout: Duration,
}

impl PasskeyConfig {
    pub fn for_origin(
        relying_party_name: impl Into<String>,
        origin: &str,
        user_handle_secret: impl Into<Vec<u8>>,
    ) -> Result<Self, PasskeyError> {
        let origin = Url::parse(origin).map_err(|_| PasskeyError::InvalidOrigin)?;
        let relying_party_id = origin.host_str().ok_or(PasskeyError::InvalidOrigin)?.to_owned();
        Ok(Self {
            relying_party_id,
            relying_party_name: relying_party_name.into(),
            allowed_origins: vec![origin],
            user_handle_secret: user_handle_secret.into(),
            timeout: Duration::from_secs(60),
        })
    }
}

#[derive(Clone, Debug)]
pub struct PasskeyUser {
    pub id: String,
    pub username: String,
    pub display_name: String,
}

#[derive(Clone, Debug)]
pub struct StoredPasskey {
    pub credential: String,
}

#[derive(Clone, Debug)]
pub struct PasskeyCeremony {
    pub id: String,
    pub options: Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedPasskey {
    pub credential_id: String,
    pub credential: String,
}

#[derive(Clone, Debug)]
pub struct PasskeyAuthenticationRequest<'a> {
    pub ceremony_id: &'a str,
    pub user: &'a PasskeyUser,
    pub credential: Value,
    pub stored_passkey: &'a StoredPasskey,
}

#[derive(Clone)]
pub struct Passkeys {
    webauthn: Webauthn,
    user_handle_secret: Arc<[u8]>,
    timeout: Duration,
    registrations: Arc<Mutex<HashMap<String, PendingRegistration>>>,
    authentications: Arc<Mutex<HashMap<String, PendingAuthentication>>>,
}

impl Debug for Passkeys {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Passkeys").finish_non_exhaustive()
    }
}

impl Passkeys {
    pub fn new(config: PasskeyConfig) -> Result<Self, PasskeyError> {
        let (origin, additional_origins) =
            config.allowed_origins.split_first().ok_or(PasskeyError::MissingAllowedOrigin)?;
        if config.user_handle_secret.is_empty() {
            return Err(PasskeyError::EmptyUserHandleSecret);
        }
        let mut builder = WebauthnBuilder::new(&config.relying_party_id, origin)?
            .rp_name(&config.relying_party_name)
            .timeout(config.timeout);
        for origin in additional_origins {
            builder = builder.append_allowed_origin(origin);
        }
        Ok(Self {
            webauthn: builder.build()?,
            user_handle_secret: config.user_handle_secret.into(),
            timeout: config.timeout,
            registrations: Arc::default(),
            authentications: Arc::default(),
        })
    }

    pub fn user_handle(&self, user_id: &str) -> Result<Uuid, PasskeyError> {
        let mut mac =
            HmacSha256::new_from_slice(&self.user_handle_secret).map_err(|_| PasskeyError::InvalidUserHandleSecret)?;
        mac.update(user_id.as_bytes());
        let digest = mac.finalize().into_bytes();
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        Ok(Uuid::from_bytes(bytes))
    }

    pub fn start_registration(
        &self,
        user: &PasskeyUser,
        existing_passkeys: &[StoredPasskey],
    ) -> Result<PasskeyCeremony, PasskeyError> {
        let handle = self.user_handle(&user.id)?;
        let excluded = credentials(existing_passkeys)?;
        let (mut options, state) = self.webauthn.start_passkey_registration(
            handle,
            &user.username,
            &user.display_name,
            (!excluded.is_empty()).then_some(excluded),
        )?;
        let mut options = serde_json::to_value(&mut options)?;
        options["publicKey"]["authenticatorSelection"]["residentKey"] = json!("required");
        options["publicKey"]["authenticatorSelection"]["requireResidentKey"] = json!(true);
        let id = Uuid::new_v4().to_string();
        let mut registrations = self.registration_states()?;
        registrations.retain(|_, pending| pending.created_at.elapsed() <= self.timeout);
        registrations
            .insert(id.clone(), PendingRegistration { created_at: Instant::now(), user_handle: handle, state });
        Ok(PasskeyCeremony { id, options })
    }

    pub fn finish_registration(
        &self,
        ceremony_id: &str,
        user: &PasskeyUser,
        credential: Value,
    ) -> Result<VerifiedPasskey, PasskeyError> {
        let pending = self.registration_states()?.remove(ceremony_id).ok_or(PasskeyError::MissingCeremony)?;
        if pending.created_at.elapsed() > self.timeout || pending.user_handle != self.user_handle(&user.id)? {
            return Err(PasskeyError::ExpiredCeremony);
        }
        let credential: RegisterPublicKeyCredential = serde_json::from_value(credential)?;
        let passkey = self.webauthn.finish_passkey_registration(&credential, &pending.state)?;
        verified_passkey(&passkey)
    }

    pub fn start_login(&self) -> Result<PasskeyCeremony, PasskeyError> {
        let (mut options, state) = self.webauthn.start_discoverable_authentication()?;
        options.mediation = None;
        let id = Uuid::new_v4().to_string();
        let mut authentications = self.authentication_states()?;
        authentications.retain(|_, pending| pending.created_at.elapsed() <= self.timeout);
        authentications.insert(
            id.clone(),
            PendingAuthentication { created_at: Instant::now(), state: AuthenticationState::Discoverable(state) },
        );
        Ok(PasskeyCeremony { id, options: serde_json::to_value(options)? })
    }

    pub fn start_confirmation(
        &self,
        user: &PasskeyUser,
        passkeys: &[StoredPasskey],
    ) -> Result<PasskeyCeremony, PasskeyError> {
        let passkeys = deserialize_passkeys(passkeys)?;
        if passkeys.is_empty() {
            return Err(PasskeyError::NoPasskeys);
        }
        let (options, state) = self.webauthn.start_passkey_authentication(&passkeys)?;
        let id = Uuid::new_v4().to_string();
        let mut authentications = self.authentication_states()?;
        authentications.retain(|_, pending| pending.created_at.elapsed() <= self.timeout);
        authentications.insert(
            id.clone(),
            PendingAuthentication {
                created_at: Instant::now(),
                state: AuthenticationState::Known { user_handle: self.user_handle(&user.id)?, state },
            },
        );
        Ok(PasskeyCeremony { id, options: serde_json::to_value(options)? })
    }

    pub fn credential_id(&self, credential: Value) -> Result<String, PasskeyError> {
        let credential: PublicKeyCredential = serde_json::from_value(credential)?;
        Ok(URL_SAFE_NO_PAD.encode(credential.get_credential_id()))
    }

    pub fn finish_authentication(
        &self,
        authentication: PasskeyAuthenticationRequest<'_>,
    ) -> Result<VerifiedPasskey, PasskeyError> {
        let pending =
            self.authentication_states()?.remove(authentication.ceremony_id).ok_or(PasskeyError::MissingCeremony)?;
        if pending.created_at.elapsed() > self.timeout {
            return Err(PasskeyError::ExpiredCeremony);
        }
        let credential: PublicKeyCredential = serde_json::from_value(authentication.credential)?;
        let mut passkey: Passkey = serde_json::from_str(&authentication.stored_passkey.credential)?;
        let result = match pending.state {
            AuthenticationState::Discoverable(state) => {
                let (handle, credential_id) = self.webauthn.identify_discoverable_authentication(&credential)?;
                if handle != self.user_handle(&authentication.user.id)? || credential_id != passkey.cred_id() {
                    return Err(PasskeyError::PasskeyDoesNotBelongToUser);
                }
                self.webauthn.finish_discoverable_authentication_v2(
                    &credential,
                    state,
                    DiscoverableKey::from(&passkey),
                )?
            }
            AuthenticationState::Known { user_handle, state } => {
                if user_handle != self.user_handle(&authentication.user.id)? {
                    return Err(PasskeyError::PasskeyDoesNotBelongToUser);
                }
                self.webauthn.finish_passkey_authentication(&credential, &state)?
            }
        };
        passkey.update_credential(&result);
        verified_passkey(&passkey)
    }

    fn registration_states(&self) -> Result<MutexGuard<'_, HashMap<String, PendingRegistration>>, PasskeyError> {
        self.registrations.lock().map_err(|_| PasskeyError::StateUnavailable)
    }

    fn authentication_states(&self) -> Result<MutexGuard<'_, HashMap<String, PendingAuthentication>>, PasskeyError> {
        self.authentications.lock().map_err(|_| PasskeyError::StateUnavailable)
    }
}

#[derive(Debug)]
struct PendingRegistration {
    created_at: Instant,
    user_handle: Uuid,
    state: PasskeyRegistration,
}

#[derive(Debug)]
struct PendingAuthentication {
    created_at: Instant,
    state: AuthenticationState,
}

#[derive(Debug)]
enum AuthenticationState {
    Discoverable(DiscoverableAuthentication),
    Known { user_handle: Uuid, state: PasskeyAuthentication },
}

#[derive(Debug, Error)]
pub enum PasskeyError {
    #[error("the WebAuthn origin is invalid")]
    InvalidOrigin,
    #[error("at least one WebAuthn origin must be allowed")]
    MissingAllowedOrigin,
    #[error("the passkey user-handle secret cannot be empty")]
    EmptyUserHandleSecret,
    #[error("the passkey user-handle secret is invalid")]
    InvalidUserHandleSecret,
    #[error("the passkey ceremony is missing or was already used")]
    MissingCeremony,
    #[error("the passkey ceremony expired or belongs to another user")]
    ExpiredCeremony,
    #[error("the passkey does not belong to this user")]
    PasskeyDoesNotBelongToUser,
    #[error("the user has no registered passkeys")]
    NoPasskeys,
    #[error("passkey ceremony state is unavailable")]
    StateUnavailable,
    #[error(transparent)]
    Webauthn(#[from] WebauthnError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn credentials(passkeys: &[StoredPasskey]) -> Result<Vec<CredentialID>, PasskeyError> {
    Ok(deserialize_passkeys(passkeys)?.into_iter().map(|passkey| passkey.cred_id().clone()).collect())
}

fn deserialize_passkeys(passkeys: &[StoredPasskey]) -> Result<Vec<Passkey>, PasskeyError> {
    passkeys.iter().map(|passkey| serde_json::from_str(&passkey.credential).map_err(PasskeyError::from)).collect()
}

fn verified_passkey(passkey: &Passkey) -> Result<VerifiedPasskey, PasskeyError> {
    Ok(VerifiedPasskey {
        credential_id: URL_SAFE_NO_PAD.encode(passkey.cred_id()),
        credential: serde_json::to_string(&passkey)?,
    })
}
