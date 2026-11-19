use std::error::Error;

use aurelin_fortify::{TwoFactorAuthentication, TwoFactorChallenge, TwoFactorChallengeOutcome, TwoFactorState};
use totp_rs::{Builder, Secret};

#[test]
fn setup_confirmation_and_challenges_preserve_fortifys_two_factor_workflow() -> Result<(), Box<dyn Error>> {
    let two_factor = TwoFactorAuthentication::new(b"application-key", "Aurelin")?;
    let setup = two_factor.enable()?;
    let secret = Secret::try_from_base32(two_factor.secret_key(&setup.encrypted_secret)?)?;
    let totp = Builder::new().with_secret(secret).build()?;
    let timestamp = 1_800_000_000;
    let code = totp.generate(timestamp).to_string();

    assert!(two_factor.confirm(&setup.encrypted_secret, &code, timestamp)?);
    assert!(!two_factor.confirm(&setup.encrypted_secret, &code, timestamp)?);
    let next_code = totp.generate(timestamp + 30).to_string();
    assert_eq!(
        two_factor.challenge(
            TwoFactorState {
                encrypted_secret: &setup.encrypted_secret,
                encrypted_recovery_codes: &setup.encrypted_recovery_codes,
            },
            TwoFactorChallenge { code: Some(&next_code), recovery_code: None },
            timestamp + 30,
        )?,
        TwoFactorChallengeOutcome::CodeAccepted
    );
    Ok(())
}

#[test]
fn a_recovery_code_is_rotated_after_one_successful_use() -> Result<(), Box<dyn Error>> {
    let two_factor = TwoFactorAuthentication::new(b"application-key", "Aurelin")?;
    let setup = two_factor.enable()?;
    let recovery_code = two_factor.recovery_codes(&setup.encrypted_recovery_codes)?[0].clone();
    let outcome = two_factor.challenge(
        TwoFactorState {
            encrypted_secret: &setup.encrypted_secret,
            encrypted_recovery_codes: &setup.encrypted_recovery_codes,
        },
        TwoFactorChallenge { code: None, recovery_code: Some(&recovery_code) },
        1_800_000_000,
    )?;
    let TwoFactorChallengeOutcome::RecoveryCodeAccepted { encrypted_recovery_codes } = outcome else {
        return Err("recovery code was rejected".into());
    };

    assert_eq!(two_factor.recovery_codes(&encrypted_recovery_codes)?.len(), 8);
    assert_eq!(
        two_factor.challenge(
            TwoFactorState {
                encrypted_secret: &setup.encrypted_secret,
                encrypted_recovery_codes: &encrypted_recovery_codes,
            },
            TwoFactorChallenge { code: None, recovery_code: Some(&recovery_code) },
            1_800_000_000,
        )?,
        TwoFactorChallengeOutcome::Invalid
    );
    Ok(())
}
