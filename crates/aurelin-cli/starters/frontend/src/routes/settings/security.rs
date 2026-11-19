use aurelin_framework::{
    axum::{extract::State, response::Response},
    inertia::{Inertia, axum::InertiaRequest},
};
use axum_tower_sessions_csrf::get_or_create_token;
use serde::Serialize;

use super::{SettingsAuth, take_errors, unauthenticated_destination, verified_user};
use crate::{
    auth::{AuthSession, PublicUser},
    frontend::Frontend,
    frontend_config::SECURITY_SETTINGS_COMPONENT,
    routes::{
        session_feedback::{FLASH_STATUS, ValidationErrors},
        web_error::WebError,
    },
};
use aurelin_fortify::axum::{PASSWORD_CONFIRMATION_REDIRECT, password_is_confirmed};

#[derive(Serialize)]
struct SecurityProps {
    auth: SettingsAuth,
    errors: ValidationErrors,
    status: Option<String>,
    csrf_token: String,
    #[serde(rename = "passwordRules")]
    password_rules: &'static str,
    #[serde(rename = "canManagePasskeys")]
    can_manage_passkeys: bool,
    passkeys: Vec<SecurityPasskey>,
    #[serde(rename = "canManageTwoFactor")]
    can_manage_two_factor: bool,
    #[serde(rename = "twoFactorEnabled")]
    two_factor_enabled: bool,
    #[serde(rename = "requiresConfirmation")]
    requires_confirmation: bool,
    #[serde(rename = "twoFactorAwaitingConfirmation")]
    two_factor_awaiting_confirmation: bool,
    #[serde(rename = "twoFactorSecretKey")]
    two_factor_secret_key: Option<String>,
    #[serde(rename = "twoFactorQrCode")]
    two_factor_qr_code: Option<String>,
    #[serde(rename = "twoFactorRecoveryCodes")]
    two_factor_recovery_codes: Vec<String>,
}

#[derive(Serialize)]
struct SecurityPasskey {
    id: i64,
    name: String,
    created_at: i64,
    last_used_at: Option<i64>,
}

pub(super) async fn show(
    State(frontend): State<Frontend>,
    auth: AuthSession,
    request: InertiaRequest,
) -> Result<Response, WebError> {
    let Some(user) = verified_user(&auth) else {
        return Ok(request.redirect(Inertia::redirect(unauthenticated_destination(&auth)))?);
    };
    if !password_is_confirmed(&auth).await? {
        auth.session
            .insert(PASSWORD_CONFIRMATION_REDIRECT, "/settings/security")
            .await?;
        return Ok(request.redirect(Inertia::redirect("/user/confirm-password"))?);
    }
    let csrf_token = get_or_create_token(&auth.session)
        .await
        .map_err(WebError::Csrf)?;
    let errors = take_errors(&auth).await?;
    let status = auth.session.remove::<String>(FLASH_STATUS).await?;
    let passkeys = auth
        .backend
        .passkeys_for_user(user.id)
        .await?
        .into_iter()
        .map(|passkey| SecurityPasskey {
            id: passkey.id,
            name: passkey.name,
            created_at: passkey.created_at,
            last_used_at: passkey.last_used_at,
        })
        .collect();
    let two_factor_enabled = user.two_factor_confirmed_at.is_some();
    Ok(request.render(
        Inertia::response(
            SECURITY_SETTINGS_COMPONENT,
            SecurityProps {
                auth: SettingsAuth {
                    user: PublicUser::from(user),
                },
                errors,
                status,
                csrf_token,
                password_rules: auth.backend.password_rules(),
                can_manage_passkeys: true,
                passkeys,
                can_manage_two_factor: true,
                two_factor_enabled,
                requires_confirmation: true,
                two_factor_awaiting_confirmation: user.two_factor_secret.is_some()
                    && !two_factor_enabled,
                two_factor_secret_key: auth.backend.two_factor_secret_key(user)?,
                two_factor_qr_code: auth.backend.two_factor_qr_code(user)?,
                two_factor_recovery_codes: if two_factor_enabled {
                    auth.backend.two_factor_recovery_codes(user)?
                } else {
                    Vec::new()
                },
            },
        ),
        move |context| frontend.document(context),
    )?)
}
