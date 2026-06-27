use crate::{config::AppConfig, error::AppError};
use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};
use argon2::password_hash::{rand_core::OsRng, SaltString};

/// Precomputed argon2id PHC hash used as a timing dummy for unknown users.
/// Verifying against this always returns `false` (wrong password),
/// but executes a full argon2 comparison to prevent timing-based user enumeration.
///
/// Params: m=19456, t=2, p=1 (OWASP minimum).
pub const DUMMY_HASH: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$cazDn9PkVEjbG3P32KDUMw$7i+0ilTgk4TPmVkdJC2PGEo2DsHz7jDW8P3nanCwm7o";

/// Hash a plaintext password using argon2id with the given config params.
///
/// # Panics / errors
/// Maps any argon2 error to `AppError::Internal`.
///
/// # Caller contract
/// Callers MUST ensure `plain.len() <= 128` before calling this function.
pub fn hash(cfg: &AppConfig, plain: &str) -> Result<String, AppError> {
    let params = Params::new(cfg.argon2_m_cost, cfg.argon2_t_cost, cfg.argon2_p_cost, None)
        .map_err(AppError::internal)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2
        .hash_password(plain.as_bytes(), &salt)
        .map_err(AppError::internal)?;
    Ok(hash.to_string())
}

/// Verify a plaintext password against an argon2 PHC hash string.
///
/// Returns `false` on ANY error (wrong password, bad hash format, etc.).
/// Never panics.
pub fn verify(hash: &str, plain: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify_roundtrip() {
        let cfg = AppConfig::test_default();
        let h = hash(&cfg, "correct horse").unwrap();
        assert!(verify(&h, "correct horse"));
        assert!(!verify(&h, "wrong"));
    }

    #[test]
    fn dummy_hash_verifies_false_but_runs() {
        assert!(!verify(DUMMY_HASH, "anything"));
    }

    #[test]
    fn dummy_hash_is_valid_phc_string() {
        assert!(
            PasswordHash::new(DUMMY_HASH).is_ok(),
            "DUMMY_HASH must be a valid PHC string"
        );
    }
}
