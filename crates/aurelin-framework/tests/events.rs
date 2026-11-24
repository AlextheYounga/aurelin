use std::{
    convert::Infallible,
    error::Error,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use aurelin_framework::events::{DispatchOutcome, EventDispatcher, ListenerControl};
use tokio::task::yield_now;

#[derive(Debug)]
struct RecordedEvent {
    value: &'static str,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[tokio::test]
async fn listeners_receive_the_typed_event_in_registration_order() -> Result<(), Box<dyn Error>> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let first_calls = Arc::clone(&calls);
    let second_calls = Arc::clone(&calls);
    let mut builder = EventDispatcher::builder();
    builder.listen::<RecordedEvent, _, _, _, _>(move |event| {
        let calls = Arc::clone(&first_calls);
        async move {
            lock(&calls).push(("first", event.value));
            Ok::<(), Infallible>(())
        }
    });
    builder.listen::<RecordedEvent, _, _, _, _>(move |event| {
        let calls = Arc::clone(&second_calls);
        async move {
            lock(&calls).push(("second", event.value));
            Ok::<(), Infallible>(())
        }
    });

    let outcome = builder.build().dispatch(RecordedEvent { value: "payload" }).await?;

    assert_eq!(outcome, DispatchOutcome::Completed);
    assert_eq!(*lock(&calls), [("first", "payload"), ("second", "payload")]);
    Ok(())
}

#[tokio::test]
async fn listeners_share_the_same_immutable_event_instance() -> Result<(), Box<dyn Error>> {
    let observed = Arc::new(Mutex::new(None));
    let first_observed = Arc::clone(&observed);
    let second_observed = Arc::clone(&observed);
    let mut builder = EventDispatcher::builder();
    builder.listen::<RecordedEvent, _, _, _, _>(move |event| {
        let observed = Arc::clone(&first_observed);
        async move {
            *lock(&observed) = Some(event);
            Ok::<(), Infallible>(())
        }
    });
    builder.listen::<RecordedEvent, _, _, _, _>(move |event| {
        let observed = Arc::clone(&second_observed);
        async move {
            let first = lock(&observed);
            assert!(first.as_ref().is_some_and(|first| Arc::ptr_eq(first, &event)));
            Ok::<(), Infallible>(())
        }
    });

    builder.build().dispatch(RecordedEvent { value: "shared" }).await?;
    Ok(())
}

#[tokio::test]
async fn an_event_without_listeners_completes_successfully() -> Result<(), Box<dyn Error>> {
    let dispatcher = EventDispatcher::builder().build();

    let outcome = dispatcher.dispatch(RecordedEvent { value: "unobserved" }).await?;

    assert_eq!(outcome, DispatchOutcome::Completed);
    Ok(())
}

#[tokio::test]
async fn event_types_do_not_receive_one_anothers_listeners() -> Result<(), Box<dyn Error>> {
    struct OtherEvent;

    let recorded_calls = Arc::new(Mutex::new(0));
    let observed_calls = Arc::clone(&recorded_calls);
    let mut builder = EventDispatcher::builder();
    builder.listen::<RecordedEvent, _, _, _, _>(move |_event| {
        let calls = Arc::clone(&observed_calls);
        async move {
            *lock(&calls) += 1;
            Ok::<(), Infallible>(())
        }
    });
    let dispatcher = builder.build();

    dispatcher.dispatch(OtherEvent).await?;

    assert_eq!(*lock(&recorded_calls), 0);
    Ok(())
}

#[derive(Debug, thiserror::Error)]
#[error("listener could not persist its work")]
struct ListenerFailure;

#[tokio::test]
async fn listener_failure_is_returned_and_prevents_later_listeners() -> Result<(), Box<dyn Error>> {
    let later_calls = Arc::new(Mutex::new(0));
    let observed_calls = Arc::clone(&later_calls);
    let mut builder = EventDispatcher::builder();
    builder.listen::<RecordedEvent, _, _, _, _>(|_event| async { Err::<(), _>(ListenerFailure) });
    builder.listen::<RecordedEvent, _, _, _, _>(move |_event| {
        let calls = Arc::clone(&observed_calls);
        async move {
            *lock(&calls) += 1;
            Ok::<(), Infallible>(())
        }
    });

    let error = builder
        .build()
        .dispatch(RecordedEvent { value: "failure" })
        .await
        .expect_err("listener failures must cross the dispatch boundary");

    assert!(error.event_type().ends_with("RecordedEvent"));
    assert_eq!(error.listener_position(), 1);
    assert_eq!(error.to_string(), format!("listener 1 for event [{}] failed: {ListenerFailure}", error.event_type()));
    assert_eq!(*lock(&later_calls), 0);
    Ok(())
}

#[tokio::test]
async fn cloned_dispatchers_share_the_configured_registrations() -> Result<(), Box<dyn Error>> {
    let values = Arc::new(Mutex::new(Vec::new()));
    let observed_values = Arc::clone(&values);
    let mut builder = EventDispatcher::builder();
    builder.listen::<RecordedEvent, _, _, _, _>(move |event| {
        let values = Arc::clone(&observed_values);
        async move {
            lock(&values).push(event.value);
            Ok::<(), Infallible>(())
        }
    });
    let dispatcher = builder.build();
    let clone = dispatcher.clone();

    dispatcher.dispatch(RecordedEvent { value: "original" }).await?;
    clone.dispatch(RecordedEvent { value: "clone" }).await?;

    assert_eq!(*lock(&values), ["original", "clone"]);
    Ok(())
}

#[derive(Debug)]
struct CountEvent(Arc<Mutex<usize>>);

async fn count_event(event: Arc<CountEvent>) -> Result<(), Infallible> {
    *lock(&event.0) += 1;
    Ok(())
}

#[tokio::test]
async fn duplicate_listener_registrations_execute_independently() -> Result<(), Box<dyn Error>> {
    let count = Arc::new(Mutex::new(0));
    let mut builder = EventDispatcher::builder();
    builder.listen::<CountEvent, _, _, _, _>(count_event);
    builder.listen::<CountEvent, _, _, _, _>(count_event);

    builder.build().dispatch(CountEvent(Arc::clone(&count))).await?;

    assert_eq!(*lock(&count), 2);
    Ok(())
}

#[tokio::test]
async fn explicit_stop_prevents_later_listener_execution() -> Result<(), Box<dyn Error>> {
    let later_calls = Arc::new(Mutex::new(0));
    let observed_calls = Arc::clone(&later_calls);
    let mut builder = EventDispatcher::builder();
    builder.listen::<RecordedEvent, _, _, _, _>(|_event| async {
        Ok::<ListenerControl, Infallible>(ListenerControl::Stop)
    });
    builder.listen::<RecordedEvent, _, _, _, _>(move |_event| {
        let calls = Arc::clone(&observed_calls);
        async move {
            *lock(&calls) += 1;
            Ok::<(), Infallible>(())
        }
    });

    let outcome = builder.build().dispatch(RecordedEvent { value: "stop" }).await?;

    assert_eq!(outcome, DispatchOutcome::Stopped);
    assert_eq!(*lock(&later_calls), 0);
    Ok(())
}

#[derive(Debug)]
struct ConcurrentEvent {
    dispatch: usize,
    payload: usize,
}

#[tokio::test]
async fn concurrent_dispatches_do_not_cross_contaminate_event_data() -> Result<(), Box<dyn Error>> {
    let received = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&received);
    let mut builder = EventDispatcher::builder();
    builder.listen::<ConcurrentEvent, _, _, _, _>(move |event| {
        let received = Arc::clone(&observed);
        async move {
            yield_now().await;
            lock(&received).push((event.dispatch, event.payload));
            Ok::<(), Infallible>(())
        }
    });
    let dispatcher = builder.build();
    let mut dispatches = Vec::new();
    for dispatch in 0..32 {
        let dispatcher = dispatcher.clone();
        dispatches.push(tokio::spawn(async move {
            dispatcher.dispatch(ConcurrentEvent { dispatch, payload: dispatch * 10 }).await
        }));
    }

    for dispatch in dispatches {
        assert_eq!(dispatch.await??, DispatchOutcome::Completed);
    }
    let mut received = lock(&received).clone();
    received.sort_unstable();
    let expected = (0..32).map(|dispatch| (dispatch, dispatch * 10)).collect::<Vec<_>>();
    assert_eq!(received, expected);
    Ok(())
}
