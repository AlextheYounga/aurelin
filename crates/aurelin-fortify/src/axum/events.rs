//! Observational authentication events. Payloads deliberately exclude credentials and requests.
//!
//! Listeners are configured through `FortifyBackend::authentication_events` using the
//! framework dispatcher. Listener errors or stopped propagation never change the
//! authentication outcome; an error prevents later listeners for that event.

use aurelin_framework::events::EventDispatcher;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Login {
    pub user_id: i64,
    pub remember: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failed;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lockout;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registered {
    pub user_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verified {
    pub user_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Logout {
    pub user_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasswordReset {
    pub email: String,
}

pub(super) async fn observe<E: Send + Sync + 'static>(dispatcher: Option<&EventDispatcher>, event: E) {
    if let Some(dispatcher) = dispatcher {
        if dispatcher.dispatch(event).await.is_err() {
            tracing::error!("authentication event listener failed");
        }
    }
}
