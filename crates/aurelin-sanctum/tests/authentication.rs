mod support;

use std::error::Error;

use aurelin_sanctum::{
    Abilities, AbilityRequirement, AuthenticatedToken, IssueToken, TokenService, authenticate_bearer, require_abilities,
};
use axum::{
    Router,
    body::Body,
    extract::Extension,
    http::{Request, StatusCode, header::AUTHORIZATION},
    middleware,
    routing::get,
};
use support::Repository;
use tower::ServiceExt;

#[tokio::test]
async fn sqlite_issuance_authentication_expiry_and_owner_scoped_revocation() -> Result<(), Box<dyn Error>> {
    let repository = Repository::new().await?;
    let service = TokenService::new(repository.clone());
    let abilities = Abilities::new(vec!["read".into()]);
    let make =
        |owner| IssueToken { owner, name: "test", abilities: &abilities, expires_at: Some(200), allow_wildcard: false };
    let token = service.issue(make(&1)).await?;
    let id = token.id;
    let plaintext = token.into_plaintext();
    let digest = repository.stored_digest(id).await?.ok_or("missing token")?;
    assert!(!plaintext.contains(&digest));
    assert_eq!(digest.len(), 64);
    let listed = service.list(&1).await?;
    assert_eq!(listed.len(), 1);
    assert!(!format!("{listed:?}").contains(&plaintext));
    assert!(service.list(&2).await?.is_empty());
    assert!(!service.revoke(&2, id).await?);
    assert!(service.authenticate(&plaintext, 199).await.is_ok());
    assert_eq!(service.list(&1).await?[0].last_used_at, Some(199));
    assert!(service.authenticate(&plaintext, 200).await.is_err());
    assert_eq!(service.list(&1).await?[0].last_used_at, Some(199));
    assert!(service.revoke(&1, id).await?);
    assert!(service.authenticate(&plaintext, 199).await.is_err());
    assert!(
        service
            .issue(IssueToken {
                owner: &1,
                name: "wild",
                abilities: &Abilities::default(),
                expires_at: None,
                allow_wildcard: false
            })
            .await
            .is_err()
    );
    Ok(())
}

async fn owner(Extension(token): Extension<AuthenticatedToken<i64>>) -> String {
    token.owner.to_string()
}

#[tokio::test]
async fn concurrent_authentication_and_revocation_never_allow_use_after_revocation() -> Result<(), Box<dyn Error>> {
    let service = TokenService::new(Repository::new().await?);
    let abilities = Abilities::new(vec!["read".into()]);
    let token = service
        .issue(IssueToken { owner: &1, name: "test", abilities: &abilities, expires_at: None, allow_wildcard: false })
        .await?;
    let id = token.id;
    let plaintext = token.into_plaintext();
    let (authentication, revoked) = tokio::join!(service.authenticate(&plaintext, 1), service.revoke(&1, id));
    assert!(revoked?);
    if let Ok(identity) = authentication {
        assert_eq!(identity.owner, 1);
    }
    assert!(service.authenticate(&plaintext, 2).await.is_err());
    Ok(())
}

#[tokio::test]
async fn bearer_and_ability_layers_return_distinct_statuses_and_reject_ambiguous_headers() -> Result<(), Box<dyn Error>>
{
    let service = TokenService::new(Repository::new().await?);
    let abilities = Abilities::new(vec!["read".into()]);
    let credential = service
        .issue(IssueToken { owner: &1, name: "test", abilities: &abilities, expires_at: None, allow_wildcard: false })
        .await?
        .into_plaintext();
    let app = Router::new()
        .route("/", get(owner))
        .layer(middleware::from_fn_with_state(AbilityRequirement::All(vec!["write".into()]), require_abilities::<i64>))
        .layer(middleware::from_fn_with_state(service, authenticate_bearer::<Repository>));
    let response = app.clone().oneshot(Request::builder().uri("/?token=secret").body(Body::empty())?).await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/?access_token=secret")
                .header(AUTHORIZATION, format!("Bearer {credential}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .header(AUTHORIZATION, format!("Bearer {credential}"))
                .header(AUTHORIZATION, "Bearer other")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .oneshot(Request::builder().header(AUTHORIZATION, format!("Bearer {credential}")).body(Body::empty())?)
        .await?;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    Ok(())
}
