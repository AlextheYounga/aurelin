use std::{error::Error, future::Future};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistrationInput {
    pub name: String,
    pub email: String,
    pub password: String,
}

pub trait CreatesNewUsers: Send + Sync {
    type User;
    type Error: Error + Send + Sync + 'static;

    fn create(&self, input: RegistrationInput) -> impl Future<Output = Result<Self::User, Self::Error>> + Send;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResetPasswordInput {
    pub email: String,
    pub password: String,
}

pub trait ResetsUserPasswords: Send + Sync {
    type Error: Error + Send + Sync + 'static;

    fn reset(&self, input: ResetPasswordInput) -> impl Future<Output = Result<(), Self::Error>> + Send;
}
