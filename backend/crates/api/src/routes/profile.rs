use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    auth::extractor::AuthUser,
    error::{AppError, FieldError},
    repo::{gdpr as gdpr_repo, user as user_repo},
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

/// `DELETE /api/v1/me`
///
/// RODO account deletion (spec 2026-07-13): hard-deletes personal data and
/// anonymises the user row — see [`gdpr_repo::delete_account`]. The client
/// confirms intent in the UI; the JWT stops working immediately afterwards
/// (extractor checks `deleted_at`). Returns 204.
pub async fn delete_me(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    // Zbierz ścieżki zdjęć eko PRZED skasowaniem wierszy (potem już ich nie ma).
    let photo_paths = gdpr_repo::eco_photo_paths(&state.pool, auth.id)
        .await
        .unwrap_or_default();

    gdpr_repo::delete_account(&state.pool, auth.id).await?;

    // Best-effort czyszczenie plików w buckecie `eco-photos` (RODO tail):
    // wymaga service key; błąd nie może cofnąć już wykonanego usunięcia konta,
    // więc tylko logujemy. Bez env SUPABASE_SERVICE_KEY — świadomie pomijamy.
    if !photo_paths.is_empty() {
        if let (Some(url), Some(key)) = (
            state.config.supabase_url.clone(),
            state.config.supabase_service_key.clone(),
        ) {
            let user = auth.id;
            tokio::spawn(async move {
                let endpoint = format!("{}/storage/v1/object/eco-photos", url.trim_end_matches('/'));
                let res = reqwest::Client::new()
                    .delete(&endpoint)
                    .header("apikey", &key)
                    .header(reqwest::header::AUTHORIZATION, format!("Bearer {key}"))
                    .json(&serde_json::json!({ "prefixes": photo_paths }))
                    .send()
                    .await;
                match res {
                    Ok(r) if r.status().is_success() => {}
                    Ok(r) => tracing::warn!(user = %user, status = %r.status(), "eco-photos cleanup failed"),
                    Err(e) => tracing::warn!(user = %user, error = %e, "eco-photos cleanup failed"),
                }
            });
        } else {
            tracing::warn!(user = %auth.id, "eco-photos cleanup skipped: no SUPABASE_SERVICE_KEY");
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/v1/me/export`
///
/// RODO data export (art. 20): full JSON of the user's personal data, served
/// as a download. Rate-limited 1/min per account (walks most tables).
pub async fn export_me(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Response, AppError> {
    crate::util::ratelimit::check_export_quota(auth.id)
        .map_err(|_| AppError::RateLimited)?;

    let export = gdpr_repo::export(&state.pool, auth.id).await?;
    let filename = format!(
        "seasteps-export-{}.json",
        chrono::Utc::now().format("%Y-%m-%d")
    );

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        Json(export),
    )
        .into_response())
}
