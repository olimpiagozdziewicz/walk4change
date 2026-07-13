use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use uuid::Uuid;

use crate::{auth::jwt, error::AppError, state::AppState};

/// Authenticated user identity, extracted from a JWT carried in the request.
///
/// Token lookup order:
/// 1. `Authorization: Bearer <jwt>` header.
/// 2. `wc_session` cookie (parsed manually from the `Cookie` header).
///
/// Any missing or invalid credential results in [`AppError::Unauthorized`].
pub struct AuthUser {
    pub id: Uuid,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = if let Some(auth_header) = parts.headers.get("Authorization") {
            // Authorization header is present — must be a valid Bearer token.
            let auth_str = auth_header.to_str().map_err(|_| AppError::Unauthorized)?;
            auth_str
                .strip_prefix("Bearer ")
                .map(|s| s.to_owned())
                .ok_or(AppError::Unauthorized)?
        } else {
            // Fall back to `wc_session` cookie.
            let cookie_header = parts
                .headers
                .get("Cookie")
                .and_then(|v| v.to_str().ok())
                .ok_or(AppError::Unauthorized)?;

            cookie_header
                .split(';')
                .find_map(|part| part.trim().strip_prefix("wc_session=").map(str::to_owned))
                .ok_or(AppError::Unauthorized)?
        };

        let claims = jwt::decode(&state.config, &token)?;

        // RODO delete (spec 2026-07-13): a JWT minted before account deletion
        // must stop working immediately. One indexed PK lookup per request is
        // acceptable at the current scale ceiling (~100–300 concurrent users,
        // audit 2026-07-10); revisit with a JWT claim + cache when scaling.
        if crate::repo::user::is_deleted(&state.pool, claims.sub).await? {
            return Err(AppError::Unauthorized);
        }

        Ok(AuthUser { id: claims.sub })
    }
}
