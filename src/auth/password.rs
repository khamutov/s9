//! Password hashing, verification, and policy enforcement using Argon2id.
//!
//! Parameters follow OWASP recommendations (DD auth.md §8.2):
//! m=19456 KiB, t=2, p=1.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use rand_core::OsRng;
use std::fmt;

/// OWASP-recommended Argon2id parameters (DD §8.2).
const MEMORY_COST: u32 = 19456;
const TIME_COST: u32 = 2;
const PARALLELISM: u32 = 1;

/// Minimum password length (DD §8.4).
const MIN_PASSWORD_LENGTH: usize = 8;

/// Pre-computed PHC string used by [`dummy_verify`] to burn equivalent CPU time
/// on unknown-user login attempts (DD §8.5 timing attack mitigation).
const DUMMY_PHC: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHQ$WFav9cpRYpPMEm/hvyzMnUxrNFya2aqHPy2OHO2z3vY";

#[derive(Debug)]
pub enum PasswordError {
    /// Password is shorter than [`MIN_PASSWORD_LENGTH`] characters.
    TooShort,
    /// Underlying argon2 hashing or verification error.
    Hash(argon2::password_hash::Error),
}

impl fmt::Display for PasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(
                f,
                "password must be at least {MIN_PASSWORD_LENGTH} characters"
            ),
            Self::Hash(e) => write!(f, "password hash error: {e}"),
        }
    }
}

impl std::error::Error for PasswordError {}

impl From<argon2::password_hash::Error> for PasswordError {
    fn from(e: argon2::password_hash::Error) -> Self {
        Self::Hash(e)
    }
}

/// Build an [`Argon2`] instance with OWASP parameters.
fn argon2_context() -> Argon2<'static> {
    let params = Params::new(MEMORY_COST, TIME_COST, PARALLELISM, None)
        .expect("OWASP Argon2id params are valid");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// Hash a plaintext password and return a PHC-format string.
///
/// Uses Argon2id with OWASP parameters and a random salt.
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2_context().hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

/// Verify a plaintext password against a PHC-format hash string.
///
/// Returns `Ok(true)` on match, `Ok(false)` on mismatch,
/// or propagates errors for malformed hashes.
pub fn verify_password(password: &str, phc_hash: &str) -> Result<bool, PasswordError> {
    let parsed = PasswordHash::new(phc_hash)?;
    match argon2_context().verify_password(password.as_bytes(), &parsed) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Perform a dummy password verification to equalize response timing
/// on unknown-user login attempts (DD §8.5).
///
/// Discards the result — the sole purpose is to burn equivalent CPU time.
pub fn dummy_verify(password: &str) {
    let parsed = PasswordHash::new(DUMMY_PHC).expect("DUMMY_PHC is a valid PHC string");
    let _ = argon2_context().verify_password(password.as_bytes(), &parsed);
}

/// Validate password against the password policy (DD §8.4).
///
/// Currently enforces minimum length only — no complexity requirements.
pub fn validate_policy(password: &str) -> Result<(), PasswordError> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(PasswordError::TooShort);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_produces_phc_string() {
        let hash = hash_password("test_password_123").unwrap();
        assert!(hash.starts_with("$argon2id$"), "expected PHC prefix, got: {hash}");
    }

    #[test]
    fn hash_and_verify_roundtrip() {
        let password = "correct horse battery staple";
        let hash = hash_password(password).unwrap();
        assert_eq!(verify_password(password, &hash).unwrap(), true);
    }

    #[test]
    fn verify_wrong_password() {
        let hash = hash_password("right_password").unwrap();
        assert_eq!(verify_password("wrong_password", &hash).unwrap(), false);
    }

    #[test]
    fn verify_invalid_hash() {
        let result = verify_password("anything", "not-a-valid-phc-string");
        assert!(result.is_err());
    }

    #[test]
    fn dummy_verify_does_not_panic() {
        dummy_verify("some_password");
    }

    #[test]
    fn validate_policy_accepts_8_chars() {
        assert!(validate_policy("12345678").is_ok());
    }

    #[test]
    fn validate_policy_rejects_short() {
        assert!(matches!(validate_policy("1234567"), Err(PasswordError::TooShort)));
    }

    #[test]
    fn validate_policy_accepts_long() {
        assert!(validate_policy(&"a".repeat(256)).is_ok());
    }
}
