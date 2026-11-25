use tower_sessions::{Expiry, Session, cookie::time::Duration};

#[cfg(feature = "two-factor-authentication")]
pub(super) const PENDING_REMEMBER_LOGIN: &str = "login.remember";

const SESSION_LIFETIME: Duration = Duration::minutes(120);
const REMEMBERED_LOGIN_LIFETIME: Duration = Duration::days(365 * 5);

pub(super) fn set_login_expiry(session: &Session, remember: bool) {
    let lifetime = if remember { REMEMBERED_LOGIN_LIFETIME } else { SESSION_LIFETIME };
    session.set_expiry(Some(Expiry::OnInactivity(lifetime)));
}
