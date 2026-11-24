use std::{
    any::Any,
    marker::PhantomData,
    mem,
    pin::Pin,
    task::{Context as TaskContext, Poll},
};

use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
};
use axum_login::{AuthSession, AuthnBackend};
use tower::{Layer, Service};

use super::Gate;

/// A route layer that authorizes an ability using the current `axum-login` session.
pub struct AuthorizeLayer<Backend, Context = ()>
where
    Backend: AuthnBackend,
{
    gate: Gate<Backend::User>,
    ability: String,
    context: Context,
    backend: PhantomData<fn() -> Backend>,
}

impl<Backend> AuthorizeLayer<Backend>
where
    Backend: AuthnBackend,
{
    /// Authorize a resource-free ability before the route handler runs.
    pub fn new(gate: Gate<Backend::User>, ability: impl Into<String>) -> Self {
        Self { gate, ability: ability.into(), context: (), backend: PhantomData }
    }
}

impl<Backend, Context> AuthorizeLayer<Backend, Context>
where
    Backend: AuthnBackend,
{
    /// Authorize an ability against fixed route context.
    pub fn with_context(gate: Gate<Backend::User>, ability: impl Into<String>, context: Context) -> Self {
        Self { gate, ability: ability.into(), context, backend: PhantomData }
    }
}

impl<Backend, Context> Clone for AuthorizeLayer<Backend, Context>
where
    Backend: AuthnBackend,
    Context: Clone,
{
    fn clone(&self) -> Self {
        Self {
            gate: self.gate.clone(),
            ability: self.ability.clone(),
            context: self.context.clone(),
            backend: PhantomData,
        }
    }
}

impl<S, Backend, Context> Layer<S> for AuthorizeLayer<Backend, Context>
where
    Backend: AuthnBackend,
    Context: Clone,
{
    type Service = AuthorizeService<S, Backend, Context>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthorizeService {
            inner,
            gate: self.gate.clone(),
            ability: self.ability.clone(),
            context: self.context.clone(),
            backend: PhantomData,
        }
    }
}

/// Service produced by [`AuthorizeLayer`].
pub struct AuthorizeService<S, Backend, Context>
where
    Backend: AuthnBackend,
{
    inner: S,
    gate: Gate<Backend::User>,
    ability: String,
    context: Context,
    backend: PhantomData<fn() -> Backend>,
}

impl<S, Backend, Context> Clone for AuthorizeService<S, Backend, Context>
where
    S: Clone,
    Backend: AuthnBackend,
    Context: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            gate: self.gate.clone(),
            ability: self.ability.clone(),
            context: self.context.clone(),
            backend: PhantomData,
        }
    }
}

impl<S, Backend, Context> Service<Request<Body>> for AuthorizeService<S, Backend, Context>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    Backend: AuthnBackend + Send + Sync + 'static,
    Backend::User: Clone + Send + Sync + 'static,
    Context: Any + Clone + Send + Sync + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, context: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let replacement = self.inner.clone();
        let mut inner = mem::replace(&mut self.inner, replacement);
        let gate = self.gate.clone();
        let ability = self.ability.clone();
        let context = self.context.clone();
        let Some(session) = request.extensions().get::<AuthSession<Backend>>() else {
            tracing::error!("authorization route is missing the axum-login session layer");
            return Box::pin(async move {
                Ok((StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response())
            });
        };
        let user = session.user.clone();

        Box::pin(async move {
            match gate.for_user(user.as_ref()).authorize(ability, &context).await {
                Ok(_) => inner.call(request).await,
                Err(error) => Ok(error.into_response()),
            }
        })
    }
}
