use std::{
    convert::Infallible,
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use aurelin_framework::{
    authorization::{
        AuthorizationError, AuthorizationResponse, AuthorizeLayer, Gate, GateError, Policy, PolicyRegistrar,
    },
    axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
        response::IntoResponse,
        routing::{get, post},
    },
};
use axum_login::{AuthManagerLayerBuilder, AuthSession, AuthUser, AuthnBackend};
use tower::ServiceExt;
use tower_sessions::{MemoryStore, SessionManagerLayer};

#[derive(Clone, Debug)]
struct User {
    id: i64,
    administrator: bool,
}

#[derive(Clone, Debug)]
struct Article {
    owner_id: i64,
    published: bool,
}

#[derive(Clone)]
struct ArticlePolicy;

impl Policy<User> for ArticlePolicy {
    fn before(&self, user: &User, _ability: &str) -> Option<AuthorizationResponse> {
        user.administrator.then(AuthorizationResponse::allow)
    }

    fn register(&self, policy: &mut PolicyRegistrar<'_, User, Self>) {
        policy.define("create", |_policy, _user: User, (): ()| async move { false });
        policy.define("update", |_policy, user: User, article: Article| async move {
            if user.id == article.owner_id {
                AuthorizationResponse::allow()
            } else {
                AuthorizationResponse::deny().with_message("You do not own this article.").with_code("not-owner")
            }
        });
        policy.define("view", |_policy, _user: User, article: Article| async move {
            if article.published { AuthorizationResponse::allow() } else { AuthorizationResponse::deny_as_not_found() }
        });
    }
}

fn policy_gate() -> Gate<User> {
    let mut builder = Gate::builder();
    builder.policy(ArticlePolicy);
    builder.build()
}

#[tokio::test]
async fn policies_authorize_users_and_preserve_denial_details() -> Result<(), Box<dyn Error>> {
    let gate = policy_gate();
    let owner = User { id: 1, administrator: false };
    let stranger = User { id: 2, administrator: false };
    let administrator = User { id: 3, administrator: true };
    let article = Article { owner_id: owner.id, published: false };

    assert!(gate.for_user(Some(&owner)).allows("update", &article).await?);
    assert!(gate.for_user(Some(&administrator)).allows("update", &article).await?);
    let denied = gate.for_user(Some(&stranger)).inspect("update", &article).await?;
    assert!(denied.denied());
    assert_eq!(denied.message(), Some("You do not own this article."));
    assert_eq!(denied.code(), Some("not-owner"));

    let concealed = gate.for_user(Some(&owner)).inspect("view", &article).await?;
    assert_eq!(concealed.status(), Some(StatusCode::NOT_FOUND));
    assert!(gate.for_user(Some(&administrator)).allows("create", &()).await?);
    assert!(gate.for_user(Some(&owner)).denies("create", &()).await?);
    Ok(())
}

#[tokio::test]
async fn a_policy_before_response_skips_the_ability_method() -> Result<(), Box<dyn Error>> {
    #[derive(Clone)]
    struct CountingPolicy(Arc<AtomicUsize>);

    impl Policy<User> for CountingPolicy {
        fn before(&self, user: &User, ability: &str) -> Option<AuthorizationResponse> {
            (user.administrator && ability == "delete").then(AuthorizationResponse::allow)
        }

        fn register(&self, policy: &mut PolicyRegistrar<'_, User, Self>) {
            policy.define("delete", |policy, _user: User, _article: Article| async move {
                policy.0.fetch_add(1, Ordering::SeqCst);
                false
            });
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let mut builder = Gate::<User>::builder();
    builder.policy(CountingPolicy(Arc::clone(&calls)));
    let gate = builder.build();
    let administrator = User { id: 1, administrator: true };
    let article = Article { owner_id: 2, published: false };

    assert!(gate.for_user(Some(&administrator)).allows("delete", &article).await?);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn action_authorization_prevents_denied_resource_mutation() -> Result<(), Box<dyn Error>> {
    let gate = policy_gate();
    let owner = User { id: 1, administrator: false };
    let stranger = User { id: 2, administrator: false };
    let article = Article { owner_id: owner.id, published: false };
    let mutations = AtomicUsize::new(0);

    if gate.for_user(Some(&stranger)).authorize("update", &article).await.is_ok() {
        mutations.fetch_add(1, Ordering::SeqCst);
    }
    assert_eq!(mutations.load(Ordering::SeqCst), 0);

    gate.for_user(Some(&owner)).authorize("update", &article).await?;
    mutations.fetch_add(1, Ordering::SeqCst);
    assert_eq!(mutations.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn guests_are_denied_unless_an_ability_explicitly_accepts_them() -> Result<(), Box<dyn Error>> {
    let mut builder = Gate::<User>::builder();
    builder.define("private", |_user: User, (): ()| async { true });
    builder.define_for_guests("published", |user: Option<User>, article: Article| async move {
        user.is_some() || article.published
    });
    let gate = builder.build();
    let published = Article { owner_id: 1, published: true };

    assert!(gate.for_user(None).denies("private", &()).await?);
    assert!(gate.for_user(None).allows("published", &published).await?);
    Ok(())
}

#[tokio::test]
async fn before_and_after_hooks_follow_laravel_ordering() -> Result<(), Box<dyn Error>> {
    let after_calls = Arc::new(AtomicUsize::new(0));
    let observed_after = Arc::clone(&after_calls);
    let mut builder = Gate::<User>::builder();
    builder.define("ordinary", |_user: User, (): ()| async { false });
    builder.before(|user: Option<User>, ability| async move {
        (user.is_some_and(|user| user.administrator) && ability == "ordinary").then(AuthorizationResponse::allow)
    });
    builder.after(move |_user, _ability, _result| {
        let observed_after = Arc::clone(&observed_after);
        async move {
            observed_after.fetch_add(1, Ordering::SeqCst);
            Some(AuthorizationResponse::allow())
        }
    });
    let gate = builder.build();
    let user = User { id: 1, administrator: false };
    let administrator = User { id: 2, administrator: true };

    assert!(gate.for_user(Some(&user)).denies("ordinary", &()).await?);
    assert!(gate.for_user(Some(&administrator)).allows("ordinary", &()).await?);
    assert!(gate.for_user(Some(&user)).allows("missing", &()).await?);
    assert_eq!(after_calls.load(Ordering::SeqCst), 3);
    Ok(())
}

#[tokio::test]
async fn checks_combine_abilities_and_unknown_abilities_use_the_default_denial() -> Result<(), Box<dyn Error>> {
    let mut builder = Gate::<User>::builder();
    builder.define("read", |_user: User, (): ()| async { true });
    builder.define("write", |_user: User, (): ()| async { false });
    builder.default_denial(AuthorizationResponse::deny_as_not_found().with_message("Hidden resource."));
    let gate = builder.build();
    let user = User { id: 1, administrator: false };
    let gate = gate.for_user(Some(&user));

    assert!(!gate.check(["read", "write"], &()).await?);
    assert!(gate.any(["read", "write"], &()).await?);
    assert!(!gate.none(["read", "write"], &()).await?);
    let missing = gate.inspect("missing", &()).await?;
    assert_eq!(missing.status(), Some(StatusCode::NOT_FOUND));
    assert_eq!(missing.message(), Some("Hidden resource."));
    Ok(())
}

#[tokio::test]
async fn mismatched_context_is_a_configuration_error() {
    let mut builder = Gate::<User>::builder();
    builder.define("update", |_user: User, _article: Article| async { true });
    let gate = builder.build();
    let user = User { id: 1, administrator: false };

    let error = gate
        .for_user(Some(&user))
        .inspect("update", &())
        .await
        .expect_err("the wrong context type must not become an ordinary denial");
    assert!(matches!(error, GateError::ContextMismatch { .. }));
}

#[test]
fn inline_authorization_bypasses_registered_hooks() {
    let hook_calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&hook_calls);
    let mut builder = Gate::<User>::builder();
    builder.before(move |_user, _ability| {
        let observed = Arc::clone(&observed);
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            Some(true)
        }
    });
    let gate = builder.build();
    let user = User { id: 1, administrator: false };
    let gate = gate.for_user(Some(&user));

    assert!(gate.allow_if_with_user(|user| user.id == 1).is_ok());
    assert!(gate.deny_if_with_user(|user| user.id == 2).is_ok());
    assert!(gate.deny_if_with_user(|user| user.id == 1).is_err());
    assert!(gate.allow_if(true).is_ok());
    assert_eq!(hook_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn inline_authorization_can_explicitly_handle_guests() {
    let gate = Gate::<User>::builder().build();
    let guest = gate.for_user(None);

    assert!(guest.allow_if(true).is_ok());
    assert!(guest.allow_if_with_user(|_| true).is_err());
    assert!(guest.allow_if_for_guests(|user| user.is_none()).is_ok());
    assert!(guest.deny_if(false).is_ok());
}

#[test]
fn authorization_errors_render_the_selected_status_and_message() {
    let response =
        AuthorizationError::Denied(AuthorizationResponse::deny_as_not_found().with_message("Article not found."))
            .into_response();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[derive(Clone, Debug)]
struct Backend;

#[derive(Clone, Debug)]
struct Credentials;

impl AuthUser for User {
    type Id = i64;

    fn id(&self) -> Self::Id {
        self.id
    }

    fn session_auth_hash(&self) -> &[u8] {
        b"authorization-test"
    }
}

impl AuthnBackend for Backend {
    type User = User;
    type Credentials = Credentials;
    type Error = Infallible;

    async fn authenticate(&self, _credentials: Credentials) -> Result<Option<User>, Infallible> {
        Ok(Some(test_user()))
    }

    async fn get_user(&self, user_id: &i64) -> Result<Option<User>, Infallible> {
        Ok((*user_id == 1).then(test_user))
    }
}

fn test_user() -> User {
    User { id: 1, administrator: false }
}

#[tokio::test]
async fn route_authorization_uses_the_authenticated_session_and_blocks_denied_actions() -> Result<(), Box<dyn Error>> {
    let executions = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&executions);
    let mut builder = Gate::<User>::builder();
    builder.define("mutate", |user: User, (): ()| async move { user.id == 1 });
    let gate = builder.build();
    let protected = Router::new()
        .route(
            "/protected",
            post(move || {
                let observed = Arc::clone(&observed);
                async move {
                    observed.fetch_add(1, Ordering::SeqCst);
                    StatusCode::NO_CONTENT
                }
            }),
        )
        .route_layer(AuthorizeLayer::<Backend>::new(gate, "mutate"));
    let sessions = SessionManagerLayer::new(MemoryStore::default()).with_secure(false);
    let authentication = AuthManagerLayerBuilder::new(Backend, sessions).build();
    let app = Router::new().route("/login", get(login)).merge(protected).layer(authentication);

    let denied = app.clone().oneshot(Request::post("/protected").body(Body::empty())?).await?;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(executions.load(Ordering::SeqCst), 0);

    let login = app.clone().oneshot(Request::get("/login").body(Body::empty())?).await?;
    let cookie = login
        .headers()
        .get(header::SET_COOKIE)
        .ok_or("login did not set a session cookie")?
        .to_str()?
        .split(';')
        .next()
        .ok_or("session cookie was empty")?
        .to_owned();
    let allowed = app.oneshot(Request::post("/protected").header(header::COOKIE, cookie).body(Body::empty())?).await?;
    assert_eq!(allowed.status(), StatusCode::NO_CONTENT);
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    Ok(())
}

#[tokio::test]
async fn route_authorization_reports_a_missing_authentication_layer() -> Result<(), Box<dyn Error>> {
    let mut builder = Gate::<User>::builder();
    builder.define("view", |_user: User, (): ()| async { true });
    let app = Router::new()
        .route("/protected", get(|| async { StatusCode::OK }))
        .route_layer(AuthorizeLayer::<Backend>::new(builder.build(), "view"));

    let response = app.oneshot(Request::get("/protected").body(Body::empty())?).await?;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    Ok(())
}

#[tokio::test]
async fn route_authorization_preserves_fixed_context_denials() -> Result<(), Box<dyn Error>> {
    let mut builder = Gate::<User>::builder();
    builder.define_for_guests("view", |_user: Option<User>, article: Article| async move {
        if article.published {
            AuthorizationResponse::allow()
        } else {
            AuthorizationResponse::deny_as_not_found().with_message("Article not found.")
        }
    });
    let article = Article { owner_id: 1, published: false };
    let protected = Router::new().route("/articles/1", get(|| async { StatusCode::OK })).route_layer(AuthorizeLayer::<
        Backend,
        Article,
    >::with_context(
        builder.build(),
        "view",
        article,
    ));
    let sessions = SessionManagerLayer::new(MemoryStore::default()).with_secure(false);
    let app = protected.layer(AuthManagerLayerBuilder::new(Backend, sessions).build());

    let response = app.oneshot(Request::get("/articles/1").body(Body::empty())?).await?;
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await?;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(&body[..], b"Article not found.");
    Ok(())
}

async fn login(mut auth: AuthSession<Backend>) -> StatusCode {
    match auth.login(&test_user()).await {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[tokio::test]
async fn denied_error_body_uses_laravels_default_message() -> Result<(), Box<dyn Error>> {
    let response = AuthorizationError::Denied(AuthorizationResponse::deny()).into_response();
    let body = to_bytes(response.into_body(), usize::MAX).await?;

    assert_eq!(&body[..], b"This action is unauthorized.");
    Ok(())
}

#[tokio::test]
async fn denied_error_body_preserves_a_policy_message() -> Result<(), Box<dyn Error>> {
    let response =
        AuthorizationError::Denied(AuthorizationResponse::deny().with_message("You do not own this article."))
            .into_response();
    let body = to_bytes(response.into_body(), usize::MAX).await?;

    assert_eq!(&body[..], b"You do not own this article.");
    Ok(())
}
