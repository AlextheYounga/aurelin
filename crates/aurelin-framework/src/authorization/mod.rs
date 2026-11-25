//! Application-defined gates and policies over authenticated users.
//!
//! Applications register their own typed ability contexts while Aurelin owns
//! evaluation, denial responses, hooks, and Axum enforcement.
//!
//! ```
//! use aurelin_framework::authorization::Gate;
//!
//! #[derive(Clone)]
//! struct User { id: i64 }
//! #[derive(Clone)]
//! struct Article { owner_id: i64 }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut gates = Gate::<User>::builder();
//! gates.define("update-article", |user: User, article: Article| async move {
//!     user.id == article.owner_id
//! });
//! let gates = gates.build();
//! let user = User { id: 1 };
//! let article = Article { owner_id: 1 };
//!
//! gates
//!     .for_user(Some(&user))
//!     .authorize("update-article", &article)
//!     .await?;
//! # Ok(())
//! # }
//! ```

mod gate;
mod http;
mod response;
mod user_gate;

pub use gate::{Gate, GateBuilder, GateError, GateEvaluated, Policy, PolicyRegistrar};
pub use http::AuthorizeLayer;
pub use response::{AuthorizationError, AuthorizationResponse, IntoAuthorizationResponse};
pub use user_gate::UserGate;
