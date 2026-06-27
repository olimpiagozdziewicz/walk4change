use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{config::AppConfig, error::AppError};

/// JWT claims payload.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub iat: i64,
    pub exp: i64,
}

/// Encode a new JWT for the given user.
///
/// `iat` is set to the current UTC timestamp; `exp` is `iat + cfg.jwt_ttl_secs`.
pub fn encode(cfg: &AppConfig, user_id: Uuid) -> Result<String, AppError> {
    let iat = Utc::now().timestamp();
    let exp = iat + cfg.jwt_ttl_secs;
    let claims = Claims { sub: user_id, iat, exp };

    jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(cfg.jwt_secret.as_bytes()),
    )
    .map_err(AppError::internal)
}

/// Decode and validate a JWT, returning the embedded claims.
///
/// Rejects tokens with an invalid signature, a wrong algorithm, or an expired `exp`.
/// Any decode error maps to [`AppError::Unauthorized`].
pub fn decode(cfg: &AppConfig, token: &str) -> Result<Claims, AppError> {
    let validation = Validation::new(Algorithm::HS256);

    jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(cfg.jwt_secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::Unauthorized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_then_decode_roundtrip() {
        let cfg = AppConfig::test_default();
        let id = Uuid::new_v4();
        let t = encode(&cfg, id).unwrap();
        assert_eq!(decode(&cfg, &t).unwrap().sub, id);
    }

    #[test]
    fn rejects_tampered_token() {
        let cfg = AppConfig::test_default();
        let t = encode(&cfg, Uuid::new_v4()).unwrap();
        let bad = format!("{}x", t);
        assert!(decode(&cfg, &bad).is_err());
    }

    #[test]
    fn rejects_expired_token() {
        // Use ttl of -120 so exp is well past the default 60s leeway
        let cfg = AppConfig {
            jwt_ttl_secs: -120,
            ..AppConfig::test_default()
        };
        let t = encode(&cfg, Uuid::new_v4()).unwrap();
        assert!(decode(&cfg, &t).is_err());
    }
}
