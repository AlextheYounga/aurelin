use std::{
    convert::Infallible,
    io,
    sync::{Arc, Mutex},
};

use aurelin_framework::{
    authorization::{AuthorizationResponse, Gate, GateError, GateEvaluated},
    axum::http::StatusCode,
    events::EventDispatcher,
};

#[tokio::test]
async fn gate_event_observes_final_rule_result_and_typed_context_without_changing_denial() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut events = EventDispatcher::builder();
    let captured = Arc::clone(&observed);
    events.listen(move |event: Arc<GateEvaluated<u32>>| {
        let captured = Arc::clone(&captured);
        async move {
            captured.lock().unwrap().push((
                event.user,
                event.ability.clone(),
                *event.context::<u64>().unwrap(),
                event.result.clone(),
            ));
            Ok::<_, Infallible>(())
        }
    });
    let mut gate = Gate::<u32>::builder();
    gate.events(events.build());
    gate.define("edit", |_user: u32, _context: u64| async { AuthorizationResponse::deny().with_message("denied") });
    gate.after(|_, _, _| async { Some(true) });
    let gate = gate.build();
    let result = gate.for_user(Some(&7)).inspect("edit", &42_u64).await.unwrap();
    assert!(result.denied());
    assert_eq!(result.message(), Some("denied"));
    let captured = observed.lock().unwrap();
    assert_eq!((captured[0].0, captured[0].1.as_str(), captured[0].2), (Some(7), "edit", 42));
    assert!(captured[0].3.as_ref().unwrap().denied());
}

#[tokio::test]
async fn failed_gate_event_listener_returns_error_instead_of_authorizing() {
    let mut events = EventDispatcher::builder();
    events.listen(|_: Arc<GateEvaluated<u32>>| async { Err::<(), _>(io::Error::other("listener failed")) });
    let mut gate = Gate::<u32>::builder();
    gate.events(events.build());
    gate.define("edit", |_user: u32, (): ()| async { true });
    let error = gate.build().for_user(Some(&7)).inspect("edit", &()).await.unwrap_err();
    assert!(matches!(error, GateError::Event(_)));
}

#[tokio::test]
async fn unresolved_gate_event_precedes_the_configured_default_denial() {
    let observed = Arc::new(Mutex::new(false));
    let mut events = EventDispatcher::builder();
    let captured = Arc::clone(&observed);
    events.listen(move |event: Arc<GateEvaluated<u32>>| {
        let captured = Arc::clone(&captured);
        async move {
            *captured.lock().unwrap() = event.user.is_none() && event.result.is_none();
            Ok::<_, Infallible>(())
        }
    });
    let mut gate = Gate::<u32>::builder();
    gate.events(events.build());
    gate.default_denial(AuthorizationResponse::deny_as_not_found());
    let response = gate.build().for_user(None).inspect("unknown", &()).await.unwrap();
    assert_eq!(response.status(), Some(StatusCode::NOT_FOUND));
    assert!(*observed.lock().unwrap());
}
