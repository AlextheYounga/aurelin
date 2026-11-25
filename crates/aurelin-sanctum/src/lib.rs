//! Personal access token services and bearer HTTP middleware.
//! Applications provide persistence through [`TokenRepository`] and explicitly
//! compose bearer-only routes separately from session and CSRF middleware.

mod abilities;
mod http;
mod service;
mod token;

pub use abilities::Abilities;
pub use http::{AbilityRequirement, ApiError, authenticate_bearer, require_abilities};
pub use service::{AuthenticatedToken, IssueToken, IssuedToken, ServiceError, TokenService};
pub use token::{NewToken, ParsedToken, StoredToken, TokenCreation, TokenError, TokenMetadata, TokenRepository};
