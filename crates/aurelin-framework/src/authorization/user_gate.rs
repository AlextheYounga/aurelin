use std::any::Any;

use super::{AuthorizationError, AuthorizationResponse, Gate, GateError, IntoAuthorizationResponse};

/// A request-oriented view of a gate registry for one user or guest.
pub struct UserGate<User> {
    gate: Gate<User>,
    user: Option<User>,
}

impl<User> UserGate<User>
where
    User: Clone + Send + Sync + 'static,
{
    pub(super) fn new(gate: Gate<User>, user: Option<User>) -> Self {
        Self { gate, user }
    }

    /// Inspect an ability's complete response.
    pub async fn inspect<Context>(
        &self,
        ability: impl AsRef<str>,
        context: &Context,
    ) -> Result<AuthorizationResponse, GateError>
    where
        Context: Any + Clone + Send + Sync + 'static,
    {
        self.gate.inspect(self.user.clone(), ability.as_ref(), context).await
    }

    /// Determine whether an ability is allowed.
    pub async fn allows<Context>(&self, ability: impl AsRef<str>, context: &Context) -> Result<bool, GateError>
    where
        Context: Any + Clone + Send + Sync + 'static,
    {
        Ok(self.inspect(ability, context).await?.allowed())
    }

    /// Determine whether an ability is denied.
    pub async fn denies<Context>(&self, ability: impl AsRef<str>, context: &Context) -> Result<bool, GateError>
    where
        Context: Any + Clone + Send + Sync + 'static,
    {
        Ok(!self.allows(ability, context).await?)
    }

    /// Authorize an ability or return its denial.
    pub async fn authorize<Context>(
        &self,
        ability: impl AsRef<str>,
        context: &Context,
    ) -> Result<AuthorizationResponse, AuthorizationError>
    where
        Context: Any + Clone + Send + Sync + 'static,
    {
        self.inspect(ability, context).await?.authorize()
    }

    /// Determine whether every ability is allowed for the same context.
    pub async fn check<Ability, Abilities, Context>(
        &self,
        abilities: Abilities,
        context: &Context,
    ) -> Result<bool, GateError>
    where
        Ability: AsRef<str>,
        Abilities: IntoIterator<Item = Ability>,
        Context: Any + Clone + Send + Sync + 'static,
    {
        for ability in abilities {
            if !self.allows(ability, context).await? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Determine whether any ability is allowed for the same context.
    pub async fn any<Ability, Abilities, Context>(
        &self,
        abilities: Abilities,
        context: &Context,
    ) -> Result<bool, GateError>
    where
        Ability: AsRef<str>,
        Abilities: IntoIterator<Item = Ability>,
        Context: Any + Clone + Send + Sync + 'static,
    {
        for ability in abilities {
            if self.allows(ability, context).await? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Determine whether no ability is allowed for the same context.
    pub async fn none<Ability, Abilities, Context>(
        &self,
        abilities: Abilities,
        context: &Context,
    ) -> Result<bool, GateError>
    where
        Ability: AsRef<str>,
        Abilities: IntoIterator<Item = Ability>,
        Context: Any + Clone + Send + Sync + 'static,
    {
        Ok(!self.any(abilities, context).await?)
    }

    /// Perform an inline allow check without running registered hooks.
    pub fn allow_if<Outcome>(&self, condition: Outcome) -> Result<AuthorizationResponse, AuthorizationError>
    where
        Outcome: IntoAuthorizationResponse,
    {
        condition.into_on_demand_response(true).authorize()
    }

    /// Perform an authenticated-user inline allow check without registered hooks.
    pub fn allow_if_with_user<Outcome>(
        &self,
        check: impl FnOnce(&User) -> Outcome,
    ) -> Result<AuthorizationResponse, AuthorizationError>
    where
        Outcome: IntoAuthorizationResponse,
    {
        self.user
            .as_ref()
            .map(check)
            .map_or_else(AuthorizationResponse::deny, |response| response.into_on_demand_response(true))
            .authorize()
    }

    /// Perform a guest-aware inline allow check without registered hooks.
    pub fn allow_if_for_guests<Outcome>(
        &self,
        check: impl FnOnce(Option<&User>) -> Outcome,
    ) -> Result<AuthorizationResponse, AuthorizationError>
    where
        Outcome: IntoAuthorizationResponse,
    {
        check(self.user.as_ref()).into_on_demand_response(true).authorize()
    }

    /// Perform an inline deny check without running registered hooks.
    pub fn deny_if<Outcome>(&self, condition: Outcome) -> Result<AuthorizationResponse, AuthorizationError>
    where
        Outcome: IntoAuthorizationResponse,
    {
        condition.into_on_demand_response(false).authorize()
    }

    /// Perform an authenticated-user inline deny check without registered hooks.
    pub fn deny_if_with_user<Outcome>(
        &self,
        check: impl FnOnce(&User) -> Outcome,
    ) -> Result<AuthorizationResponse, AuthorizationError>
    where
        Outcome: IntoAuthorizationResponse,
    {
        self.user
            .as_ref()
            .map(check)
            .map_or_else(AuthorizationResponse::deny, |response| response.into_on_demand_response(false))
            .authorize()
    }

    /// Perform a guest-aware inline deny check without registered hooks.
    pub fn deny_if_for_guests<Outcome>(
        &self,
        check: impl FnOnce(Option<&User>) -> Outcome,
    ) -> Result<AuthorizationResponse, AuthorizationError>
    where
        Outcome: IntoAuthorizationResponse,
    {
        check(self.user.as_ref()).into_on_demand_response(false).authorize()
    }
}
