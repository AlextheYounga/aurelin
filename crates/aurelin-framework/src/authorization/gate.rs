use std::{
    any::{Any, TypeId, type_name},
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Arc,
};

use super::{AuthorizationResponse, IntoAuthorizationResponse, UserGate};
use crate::events::{DispatchError, EventDispatcher};

/// The result of a gate check, before the default denial is applied.
pub struct GateEvaluated<User> {
    /// The user being authorized, or a guest.
    pub user: Option<User>,
    /// The checked ability.
    pub ability: String,
    /// The resolved result, or `None` if no rule resolved the check.
    pub result: Option<AuthorizationResponse>,
    context: Arc<dyn Any + Send + Sync>,
}

impl<User> GateEvaluated<User> {
    /// Recover the typed context supplied to the gate.
    pub fn context<Context: Any>(&self) -> Option<&Context> {
        self.context.downcast_ref()
    }
}

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
type AbilityCallback<User> =
    dyn Fn(Option<User>, &dyn Any) -> Result<BoxFuture<AuthorizationResponse>, GateError> + Send + Sync;
type BeforeCallback<User> = dyn Fn(Option<User>, String) -> BoxFuture<Option<AuthorizationResponse>> + Send + Sync;
type AfterCallback<User> = dyn Fn(Option<User>, String, Option<AuthorizationResponse>) -> BoxFuture<Option<AuthorizationResponse>>
    + Send
    + Sync;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct AbilityKey {
    name: String,
    context: TypeId,
}

/// An application policy that registers typed abilities and optional pre-authorization.
pub trait Policy<User>: Clone + Send + Sync + Sized + 'static {
    /// Resolve an ability before its policy method, or return `None` to continue.
    fn before(&self, _user: &User, _ability: &str) -> Option<AuthorizationResponse> {
        None
    }

    /// Register this policy's application-owned abilities.
    fn register(&self, policy: &mut PolicyRegistrar<'_, User, Self>);
}

/// Registers abilities belonging to one application policy.
pub struct PolicyRegistrar<'a, User, ApplicationPolicy> {
    gate: &'a mut GateBuilder<User>,
    policy: ApplicationPolicy,
}

impl<User, ApplicationPolicy> PolicyRegistrar<'_, User, ApplicationPolicy>
where
    User: Clone + Send + Sync + 'static,
    ApplicationPolicy: Policy<User>,
{
    /// Register an authenticated-user policy ability.
    pub fn define<Context, Check, CheckFuture, Result>(&mut self, ability: impl Into<String>, check: Check) -> &mut Self
    where
        Context: Any + Clone + Send + Sync + 'static,
        Check: Fn(ApplicationPolicy, User, Context) -> CheckFuture + Send + Sync + 'static,
        CheckFuture: Future<Output = Result> + Send + 'static,
        Result: IntoAuthorizationResponse + 'static,
    {
        let ability = ability.into();
        let callback_ability = ability.clone();
        let policy = self.policy.clone();
        let check = Arc::new(check);
        self.gate.define(ability, move |user: User, context: Context| {
            let policy = policy.clone();
            let before = policy.before(&user, &callback_ability);
            let check = Arc::clone(&check);
            async move {
                match before {
                    Some(response) => response,
                    None => check(policy, user, context).await.into_authorization_response(),
                }
            }
        });
        self
    }

    /// Register a guest-aware policy ability.
    pub fn define_for_guests<Context, Check, CheckFuture, Result>(
        &mut self,
        ability: impl Into<String>,
        check: Check,
    ) -> &mut Self
    where
        Context: Any + Clone + Send + Sync + 'static,
        Check: Fn(ApplicationPolicy, Option<User>, Context) -> CheckFuture + Send + Sync + 'static,
        CheckFuture: Future<Output = Result> + Send + 'static,
        Result: IntoAuthorizationResponse + 'static,
    {
        let ability = ability.into();
        let callback_ability = ability.clone();
        let policy = self.policy.clone();
        let check = Arc::new(check);
        self.gate.define_for_guests(ability, move |user: Option<User>, context: Context| {
            let policy = policy.clone();
            let before = user.as_ref().and_then(|user| policy.before(user, &callback_ability));
            let check = Arc::clone(&check);
            async move {
                match before {
                    Some(response) => response,
                    None => check(policy, user, context).await.into_authorization_response(),
                }
            }
        });
        self
    }
}

/// Builds an immutable authorization gate registry.
pub struct GateBuilder<User> {
    abilities: HashMap<AbilityKey, Arc<AbilityCallback<User>>>,
    before: Vec<Arc<BeforeCallback<User>>>,
    after: Vec<Arc<AfterCallback<User>>>,
    default_denial: AuthorizationResponse,
    events: Option<EventDispatcher>,
}

impl<User> Default for GateBuilder<User> {
    fn default() -> Self {
        Self {
            abilities: HashMap::new(),
            before: Vec::new(),
            after: Vec::new(),
            default_denial: AuthorizationResponse::deny(),
            events: None,
        }
    }
}

impl<User> GateBuilder<User>
where
    User: Clone + Send + Sync + 'static,
{
    /// Create an empty gate registry builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an ability that requires an authenticated user.
    pub fn define<Context, Check, CheckFuture, Result>(&mut self, ability: impl Into<String>, check: Check) -> &mut Self
    where
        Context: Any + Clone + Send + Sync + 'static,
        Check: Fn(User, Context) -> CheckFuture + Send + Sync + 'static,
        CheckFuture: Future<Output = Result> + Send + 'static,
        Result: IntoAuthorizationResponse + 'static,
    {
        self.define_for_guests(ability, move |user: Option<User>, context| {
            let check = user.map(|user| check(user, context));
            async move {
                match check {
                    Some(result) => result.await.into_authorization_response(),
                    None => AuthorizationResponse::deny(),
                }
            }
        })
    }

    /// Register an ability that explicitly receives authenticated users or guests.
    pub fn define_for_guests<Context, Check, CheckFuture, Result>(
        &mut self,
        ability: impl Into<String>,
        check: Check,
    ) -> &mut Self
    where
        Context: Any + Clone + Send + Sync + 'static,
        Check: Fn(Option<User>, Context) -> CheckFuture + Send + Sync + 'static,
        CheckFuture: Future<Output = Result> + Send + 'static,
        Result: IntoAuthorizationResponse + 'static,
    {
        let name = ability.into();
        let callback_name = name.clone();
        let callback = move |user: Option<User>, context: &dyn Any| {
            let context = context.downcast_ref::<Context>().cloned().ok_or_else(|| GateError::ContextMismatch {
                ability: callback_name.clone(),
                expected: type_name::<Context>(),
            })?;
            let future = check(user, context);
            Ok(Box::pin(async move { future.await.into_authorization_response() }) as BoxFuture<_>)
        };
        let key = AbilityKey { name, context: TypeId::of::<Context>() };
        self.abilities.insert(key, Arc::new(callback));
        self
    }

    /// Register an application-owned policy.
    pub fn policy<P>(&mut self, policy: P) -> &mut Self
    where
        P: Policy<User>,
    {
        let registration = policy.clone();
        let mut registrar = PolicyRegistrar { gate: self, policy };
        registration.register(&mut registrar);
        self
    }

    /// Register a callback that can resolve an ability before its normal check.
    pub fn before<Hook, HookFuture, Result>(&mut self, hook: Hook) -> &mut Self
    where
        Hook: Fn(Option<User>, String) -> HookFuture + Send + Sync + 'static,
        HookFuture: Future<Output = Option<Result>> + Send + 'static,
        Result: IntoAuthorizationResponse + 'static,
    {
        self.before.push(Arc::new(move |user, ability| {
            let future = hook(user, ability);
            Box::pin(async move { future.await.map(IntoAuthorizationResponse::into_authorization_response) })
        }));
        self
    }

    /// Register a callback that observes every check and may resolve an otherwise missing ability.
    pub fn after<Hook, HookFuture, Result>(&mut self, hook: Hook) -> &mut Self
    where
        Hook: Fn(Option<User>, String, Option<AuthorizationResponse>) -> HookFuture + Send + Sync + 'static,
        HookFuture: Future<Output = Option<Result>> + Send + 'static,
        Result: IntoAuthorizationResponse + 'static,
    {
        self.after.push(Arc::new(move |user, ability, response| {
            let future = hook(user, ability, response);
            Box::pin(async move { future.await.map(IntoAuthorizationResponse::into_authorization_response) })
        }));
        self
    }

    /// Select the denial used when no ability or hook resolves a check.
    pub fn default_denial(&mut self, response: AuthorizationResponse) -> &mut Self {
        self.default_denial = response;
        self
    }

    /// Dispatch gate evaluation events through the application's dispatcher.
    pub fn events(&mut self, events: EventDispatcher) -> &mut Self {
        self.events = Some(events);
        self
    }

    /// Finish configuring the immutable gate registry.
    pub fn build(&self) -> Gate<User> {
        Gate {
            inner: Arc::new(GateInner {
                abilities: self.abilities.clone(),
                before: self.before.clone(),
                after: self.after.clone(),
                default_denial: self.default_denial.clone(),
                events: self.events.clone(),
            }),
        }
    }
}

struct GateInner<User> {
    abilities: HashMap<AbilityKey, Arc<AbilityCallback<User>>>,
    before: Vec<Arc<BeforeCallback<User>>>,
    after: Vec<Arc<AfterCallback<User>>>,
    default_denial: AuthorizationResponse,
    events: Option<EventDispatcher>,
}

/// A shared registry of application-defined authorization abilities.
pub struct Gate<User> {
    inner: Arc<GateInner<User>>,
}

impl<User> Clone for Gate<User> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

impl<User> Gate<User>
where
    User: Clone + Send + Sync + 'static,
{
    /// Configure a gate registry.
    pub fn builder() -> GateBuilder<User> {
        GateBuilder::new()
    }

    /// Evaluate abilities for the supplied user or guest.
    pub fn for_user(&self, user: Option<&User>) -> UserGate<User> {
        UserGate::new(self.clone(), user.cloned())
    }

    pub(super) async fn inspect<Context>(
        &self,
        user: Option<User>,
        ability: &str,
        context: &Context,
    ) -> Result<AuthorizationResponse, GateError>
    where
        Context: Any + Clone + Send + Sync + 'static,
    {
        let mut result = None;
        for hook in &self.inner.before {
            if let Some(response) = hook(user.clone(), ability.to_owned()).await {
                result = Some(response);
                break;
            }
        }

        if result.is_none() {
            let key = AbilityKey { name: ability.to_owned(), context: TypeId::of::<Context>() };
            if let Some(callback) = self.inner.abilities.get(&key) {
                result = Some(callback(user.clone(), context)?.await);
            } else if self.inner.abilities.keys().any(|key| key.name == ability) {
                return Err(GateError::ContextMismatch {
                    ability: ability.to_owned(),
                    expected: "the context type registered for this ability",
                });
            }
        }

        for hook in &self.inner.after {
            let candidate = hook(user.clone(), ability.to_owned(), result.clone()).await;
            if result.is_none() {
                result = candidate;
            }
        }

        if let Some(events) = &self.inner.events {
            events
                .dispatch(GateEvaluated {
                    user,
                    ability: ability.to_owned(),
                    result: result.clone(),
                    context: Arc::new(context.clone()),
                })
                .await?;
        }
        Ok(result.unwrap_or_else(|| self.inner.default_denial.clone()))
    }
}

/// A gate was invoked with an incompatible typed context.
#[derive(Debug, thiserror::Error)]
pub enum GateError {
    #[error("ability [{ability}] requires {expected}")]
    ContextMismatch { ability: String, expected: &'static str },
    /// An evaluation listener failed; authorization did not complete.
    #[error(transparent)]
    Event(#[from] DispatchError),
}
