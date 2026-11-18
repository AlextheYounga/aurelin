use tower_sessions::{
    Expiry, SessionManagerLayer, SessionStore,
    cookie::{SameSite, time::Duration},
};

pub fn web_session_layer<Store>(store: Store, cookie_name: &'static str, secure: bool) -> SessionManagerLayer<Store>
where
    Store: SessionStore,
{
    SessionManagerLayer::new(store)
        .with_name(cookie_name)
        .with_secure(secure)
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::minutes(120)))
}
