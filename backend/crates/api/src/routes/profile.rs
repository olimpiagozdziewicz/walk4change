use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    auth::extractor::AuthUser,
    error::{AppError, FieldError},
    repo::user as user_repo,
    response,
    state::AppState,
};

/// `GET /api/v1/me`
///
/// Returns the authenticated user's public profile.
/// Responds 200 with `{"data": Profile}`.
/// Requires a valid JWT (Bearer or `wc_session` cookie); missing/invalid → 401.
pub async fn get_me(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Value>, AppError> {
    let profile = user_repo::get_profile(&state.pool, auth.id).await?;
    Ok(response::data(profile))
}

/// Deserialised body for `PATCH /api/v1/me`.
/// Every field is optional; absent fields are not updated.
#[derive(Deserialize)]
pub struct PatchMeRequest {
    pub display_name: Option<String>,
    /// Stored as-is; never fetched server-side (SSRF guard).
    pub avatar_url: Option<String>,
    pub bio: Option<String>,
    pub interests: Option<Vec<String>>,
}

/// `PATCH /api/v1/me`
///
/// Partially updates the authenticated user's profile.
/// Returns 200 with the updated `{"data": Profile}`.
///
/// Validation:
/// - If `display_name` is provided it must be non-empty → 422 otherwise.
///
/// Requires a valid JWT; missing/invalid → 401.
pub async fn patch_me(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<PatchMeRequest>,
) -> Result<Json<Value>, AppError> {
    let mut errors: Vec<FieldError> = Vec::new();

    // Validate: if display_name is supplied it must not be blank.
    if let Some(ref name) = body.display_name {
        if name.trim().is_empty() {
            errors.push(FieldError {
                field: "display_name".into(),
                message: "must not be empty".into(),
                code: "REQUIRED".into(),
            });
        }
        crate::util::validate::check_max_len(&mut errors, "display_name", name.trim(), 80);
    }
    if let Some(ref bio) = body.bio {
        crate::util::validate::check_max_len(&mut errors, "bio", bio, 500);
    }
    if let Some(ref interests) = body.interests {
        if interests.len() > 20 {
            errors.push(FieldError {
                field: "interests".into(),
                message: "at most 20 items".into(),
                code: "INVALID_LENGTH".into(),
            });
        }
        for it in interests {
            crate::util::validate::check_max_len(&mut errors, "interests", it, 40);
        }
    }
    crate::util::validate::check_optional_url(&mut errors, "avatar_url", body.avatar_url.as_deref());

    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }

    let patch = user_repo::ProfilePatch {
        display_name: body.display_name,
        avatar_url: body.avatar_url,
        bio: body.bio,
        interests: body.interests,
    };

    let profile = user_repo::update_profile(&state.pool, auth.id, patch).await?;
    Ok(response::data(profile))
}
