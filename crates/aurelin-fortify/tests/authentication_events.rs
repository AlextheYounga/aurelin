use std::sync::{Arc, Mutex};

use aurelin_fortify::axum::events::{Failed, Lockout, Login, Logout, PasswordReset, Registered, Verified};
use aurelin_framework::events::{DispatchOutcome, EventDispatcher};

#[tokio::test]
async fn explicitly_registered_listeners_receive_each_public_fortify_event_once_in_order() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let mut builder = EventDispatcher::builder();

    macro_rules! record {
        ($event:ty, $label:literal) => {{
            let received = Arc::clone(&received);
            builder.listen(move |_event: Arc<$event>| {
                let received = Arc::clone(&received);
                async move {
                    received.lock().unwrap().push($label);
                    Ok::<(), std::io::Error>(())
                }
            });
        }};
    }

    record!(Login, "login");
    record!(Failed, "failed");
    record!(Lockout, "lockout");
    record!(Registered, "registered");
    record!(PasswordReset, "password_reset");
    record!(Verified, "verified");
    record!(Logout, "logout");
    let dispatcher = builder.build();

    assert_eq!(dispatcher.dispatch(Login { user_id: 7, remember: true }).await.unwrap(), DispatchOutcome::Completed);
    assert_eq!(dispatcher.dispatch(Failed).await.unwrap(), DispatchOutcome::Completed);
    assert_eq!(dispatcher.dispatch(Lockout).await.unwrap(), DispatchOutcome::Completed);
    assert_eq!(dispatcher.dispatch(Registered { user_id: 7 }).await.unwrap(), DispatchOutcome::Completed);
    assert_eq!(
        dispatcher.dispatch(PasswordReset { email: "ada@example.com".into() }).await.unwrap(),
        DispatchOutcome::Completed
    );
    assert_eq!(dispatcher.dispatch(Verified { user_id: 7 }).await.unwrap(), DispatchOutcome::Completed);
    assert_eq!(dispatcher.dispatch(Logout { user_id: 7 }).await.unwrap(), DispatchOutcome::Completed);

    assert_eq!(
        *received.lock().unwrap(),
        ["login", "failed", "lockout", "registered", "password_reset", "verified", "logout"]
    );
}

#[test]
fn public_event_debug_payloads_expose_only_non_secret_identifiers_and_metadata() {
    let debug = format!(
        "{:?} {:?} {:?} {:?} {:?} {:?} {:?}",
        Login { user_id: 7, remember: true },
        Failed,
        Lockout,
        Registered { user_id: 7 },
        PasswordReset { email: "ada@example.com".into() },
        Verified { user_id: 7 },
        Logout { user_id: 7 },
    );

    for secret in [
        "plaintext-password",
        "password-confirmation",
        "reset-token",
        "csrf-session-id",
        "passkey-secret",
        "totp-secret",
        "recovery-code",
    ] {
        assert!(!debug.contains(secret), "event debug output leaked {secret}");
    }
}
