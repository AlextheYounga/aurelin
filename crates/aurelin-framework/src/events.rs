use std::{
    any::{Any, TypeId, type_name},
    collections::HashMap,
    error::Error,
    future::Future,
    pin::Pin,
    sync::Arc,
};

type BoxError = Box<dyn Error + Send + Sync + 'static>;
type ErasedEvent = Arc<dyn Any + Send + Sync>;
type ListenerFuture = Pin<Box<dyn Future<Output = Result<ListenerControl, BoxError>> + Send + 'static>>;
type Listener = dyn Fn(ErasedEvent) -> ListenerFuture + Send + Sync;

/// Controls whether dispatch continues to later listeners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListenerControl {
    /// Continue dispatching to the next registered listener.
    Continue,
    /// Stop dispatching this event without treating the stop as a failure.
    Stop,
}

impl From<()> for ListenerControl {
    fn from((): ()) -> Self {
        Self::Continue
    }
}

/// Describes how an event dispatch completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchOutcome {
    /// Every registered listener completed, including the case where none exist.
    Completed,
    /// A listener explicitly stopped propagation.
    Stopped,
}

/// Builds an immutable registry of typed event listeners.
#[derive(Default)]
pub struct EventDispatcherBuilder {
    listeners: HashMap<TypeId, Vec<Arc<Listener>>>,
}

impl EventDispatcherBuilder {
    /// Create an empty listener registry builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a typed listener after all listeners already registered for this event.
    pub fn listen<Event, Callback, CallbackFuture, Outcome, ListenerError>(&mut self, listener: Callback) -> &mut Self
    where
        Event: Send + Sync + 'static,
        Callback: Fn(Arc<Event>) -> CallbackFuture + Send + Sync + 'static,
        CallbackFuture: Future<Output = Result<Outcome, ListenerError>> + Send + 'static,
        Outcome: Into<ListenerControl> + 'static,
        ListenerError: Error + Send + Sync + 'static,
    {
        let listener = Arc::new(listener);
        let callback = move |event: ErasedEvent| {
            let listener = Arc::clone(&listener);
            Box::pin(async move {
                let event = Arc::downcast::<Event>(event)
                    .map_err(|_| Box::new(EventTypeMismatch { expected: type_name::<Event>() }) as BoxError)?;
                listener(event).await.map(Into::into).map_err(|error| Box::new(error) as BoxError)
            }) as ListenerFuture
        };

        self.listeners.entry(TypeId::of::<Event>()).or_default().push(Arc::new(callback));
        self
    }

    /// Finish configuring the immutable dispatcher.
    pub fn build(&self) -> EventDispatcher {
        EventDispatcher { inner: Arc::new(EventDispatcherInner { listeners: self.listeners.clone() }) }
    }
}

struct EventDispatcherInner {
    listeners: HashMap<TypeId, Vec<Arc<Listener>>>,
}

/// A shared, immutable registry that dispatches typed events sequentially.
#[derive(Clone)]
pub struct EventDispatcher {
    inner: Arc<EventDispatcherInner>,
}

impl EventDispatcher {
    /// Configure a new event dispatcher.
    pub fn builder() -> EventDispatcherBuilder {
        EventDispatcherBuilder::new()
    }

    /// Dispatch an event to its listeners in registration order.
    ///
    /// Listeners are awaited one at a time. A listener error is returned immediately,
    /// and later listeners are not invoked.
    pub async fn dispatch<Event>(&self, event: Event) -> Result<DispatchOutcome, DispatchError>
    where
        Event: Send + Sync + 'static,
    {
        let Some(listeners) = self.inner.listeners.get(&TypeId::of::<Event>()) else {
            return Ok(DispatchOutcome::Completed);
        };
        let event: ErasedEvent = Arc::new(event);

        for (index, listener) in listeners.iter().enumerate() {
            let control = listener(Arc::clone(&event)).await.map_err(|source| DispatchError {
                event_type: type_name::<Event>(),
                listener_position: index + 1,
                source,
            })?;
            if control == ListenerControl::Stop {
                return Ok(DispatchOutcome::Stopped);
            }
        }

        Ok(DispatchOutcome::Completed)
    }
}

/// A listener failed while handling an event.
#[derive(Debug, thiserror::Error)]
#[error("listener {listener_position} for event [{event_type}] failed: {source}")]
pub struct DispatchError {
    event_type: &'static str,
    listener_position: usize,
    #[source]
    source: BoxError,
}

impl DispatchError {
    /// The Rust type name of the dispatched event.
    pub fn event_type(&self) -> &'static str {
        self.event_type
    }

    /// The one-based position of the failing listener in registration order.
    pub fn listener_position(&self) -> usize {
        self.listener_position
    }
}

#[derive(Debug, thiserror::Error)]
#[error("event payload did not match registered type [{expected}]")]
struct EventTypeMismatch {
    expected: &'static str,
}
