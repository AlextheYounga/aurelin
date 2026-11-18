use std::{convert::Infallible, sync::Arc};

use aurelin_fortify::{CreatesNewUsers, RegistrationInput, ResetPasswordInput, ResetsUserPasswords};
use tokio::sync::Mutex;

#[derive(Clone, Default)]
struct UserActions {
    reset: Arc<Mutex<Option<ResetPasswordInput>>>,
}

impl CreatesNewUsers for UserActions {
    type User = String;
    type Error = Infallible;

    async fn create(&self, input: RegistrationInput) -> Result<Self::User, Self::Error> {
        Ok(input.email)
    }
}

impl ResetsUserPasswords for UserActions {
    type Error = Infallible;

    async fn reset(&self, input: ResetPasswordInput) -> Result<(), Self::Error> {
        *self.reset.lock().await = Some(input);
        Ok(())
    }
}

#[tokio::test]
async fn application_actions_own_user_creation_and_password_mutation() {
    let actions = UserActions::default();
    let user = actions
        .create(RegistrationInput {
            name: "Ada Lovelace".into(),
            email: "ada@example.com".into(),
            password: "secret".into(),
        })
        .await;
    assert_eq!(user.ok().as_deref(), Some("ada@example.com"));

    let input = ResetPasswordInput { email: "ada@example.com".into(), password: "new-secret".into() };
    let result = actions.reset(input.clone()).await;
    assert!(result.is_ok());
    assert_eq!(*actions.reset.lock().await, Some(input));
}
