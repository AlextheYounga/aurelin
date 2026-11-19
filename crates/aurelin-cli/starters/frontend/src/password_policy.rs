use aurelin_framework::auth::CompromisedPasswordChecker;

#[derive(Clone, Debug)]
pub struct PasswordPolicy {
    production: bool,
    compromised_passwords: CompromisedPasswordChecker,
}

impl PasswordPolicy {
    pub fn for_environment(environment: &str) -> Self {
        Self {
            production: environment == "production",
            compromised_passwords: CompromisedPasswordChecker::default(),
        }
    }

    pub fn with_compromised_password_checker(
        environment: &str,
        compromised_passwords: CompromisedPasswordChecker,
    ) -> Self {
        Self {
            production: environment == "production",
            compromised_passwords,
        }
    }

    pub async fn validate(&self, password: &str) -> Result<(), PasswordPolicyViolation> {
        let minimum_length = if self.production { 12 } else { 8 };
        if password.chars().count() < minimum_length {
            return Err(PasswordPolicyViolation::TooShort { minimum_length });
        }
        if self.production
            && (!password.chars().any(char::is_lowercase)
                || !password.chars().any(char::is_uppercase))
        {
            return Err(PasswordPolicyViolation::MixedCase);
        }
        if self.production && !password.chars().any(char::is_numeric) {
            return Err(PasswordPolicyViolation::Number);
        }
        if self.production
            && !password
                .chars()
                .any(|character| !character.is_alphanumeric() && !character.is_control())
        {
            return Err(PasswordPolicyViolation::Symbol);
        }
        if self.production {
            match self.compromised_passwords.is_compromised(password).await {
                Ok(true) => return Err(PasswordPolicyViolation::Compromised),
                Ok(false) => {}
                Err(error) => {
                    tracing::error!(%error, "compromised-password lookup failed");
                }
            }
        }
        Ok(())
    }

    pub fn browser_rules(&self) -> &'static str {
        if self.production {
            "minlength: 12; required: lower; required: upper; required: digit; required: special;"
        } else {
            "minlength: 8;"
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordPolicyViolation {
    TooShort { minimum_length: usize },
    MixedCase,
    Number,
    Symbol,
    Compromised,
}

impl PasswordPolicyViolation {
    pub fn message(self) -> String {
        match self {
            Self::TooShort { minimum_length } => {
                format!("The password must be at least {minimum_length} characters.")
            }
            Self::MixedCase => "The password must contain at least one uppercase and one lowercase letter.".into(),
            Self::Number => "The password must contain at least one number.".into(),
            Self::Symbol => "The password must contain at least one symbol.".into(),
            Self::Compromised => {
                "The given password has appeared in a data leak. Please choose a different password.".into()
            }
        }
    }
}
