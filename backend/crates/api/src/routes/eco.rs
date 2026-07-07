use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::Value;

use crate::{auth::extractor::AuthUser, error::{AppError, FieldError}, response, state::AppState};

/// One eco report row, serialised for the client.
fn row_json(
    id: uuid::Uuid,
    kind: String,
    category: String,
    description: String,
    location: String,
    status: String,
    photo_url: Option<String>,
    photo_before_url: Option<String>,
    photo_after_url: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
) -> Value {
    serde_json::json!({
        "id": id,
        "kind": kind,
        "category": category,
        "description": description,
        "location": location,
        "status": status,
        "photo_url": photo_url,
        "photo_before_url": photo_before_url,
        "photo_after_url": photo_after_url,
        "created_at": created_at,
    })
}

type EcoRow = (
    uuid::Uuid,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    chrono::DateTime<chrono::Utc>,
);

const SELECT_COLS: &str = "id, kind, category, description, location, status, \
     photo_url, photo_before_url, photo_after_url, created_at";

/// Body for `POST /api/v1/eco/reports`.
///
/// Photos are uploaded by the client directly to Supabase Storage; only their
/// public URLs are sent here (the API body cap is 64 KiB — far below a photo).
#[derive(Deserialize)]
pub struct CreateEcoRequest {
    /// `"report"` (problem) or `"cleanup"` (brag).
    pub kind: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub photo_url: Option<String>,
    #[serde(default)]
    pub photo_before_url: Option<String>,
    #[serde(default)]
    pub photo_after_url: Option<String>,
}

/// `POST /api/v1/eco/reports` — create an eco report for the authenticated user.
pub async fn create_report(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateEcoRequest>,
) -> Result<Json<Value>, AppError> {
    let kind = body.kind.trim();
    if kind != "report" && kind != "cleanup" {
        return Err(AppError::Validation(vec![FieldError {
            field: "kind".into(),
            message: "must be 'report' or 'cleanup'".into(),
            code: "INVALID".into(),
        }]));
    }
    // A report problem starts 'reported'; a cleanup brag is 'cleaned'.
    let status = if kind == "cleanup" { "cleaned" } else { "reported" };

    // Length + URL validation (security audit 2026-07-08).
    let mut errors: Vec<FieldError> = Vec::new();
    crate::util::validate::check_max_len(&mut errors, "category", body.category.trim(), 40);
    crate::util::validate::check_max_len(&mut errors, "description", body.description.trim(), 1000);
    crate::util::validate::check_max_len(&mut errors, "location", body.location.trim(), 200);
    crate::util::validate::check_optional_url(&mut errors, "photo_url", body.photo_url.as_deref());
    crate::util::validate::check_optional_url(&mut errors, "photo_before_url", body.photo_before_url.as_deref());
    crate::util::validate::check_optional_url(&mut errors, "photo_after_url", body.photo_after_url.as_deref());
    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }

    let row: EcoRow = sqlx::query_as(&format!(
        "INSERT INTO eco_reports \
            (user_id, kind, category, description, location, status, \
             photo_url, photo_before_url, photo_after_url) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
         RETURNING {SELECT_COLS}"
    ))
    .bind(auth.id)
    .bind(kind)
    .bind(body.category.trim())
    .bind(body.description.trim())
    .bind(body.location.trim())
    .bind(status)
    .bind(body.photo_url)
    .bind(body.photo_before_url)
    .bind(body.photo_after_url)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::internal)?;

    Ok(response::data(row_json(
        row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8, row.9,
    )))
}

/// `GET /api/v1/eco/reports` — recent reports across all users (community feed).
pub async fn list_reports(
    _auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Value>, AppError> {
    let rows: Vec<EcoRow> = sqlx::query_as(&format!(
        "SELECT {SELECT_COLS} FROM eco_reports ORDER BY created_at DESC LIMIT 50"
    ))
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| row_json(r.0, r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8, r.9))
        .collect();
    Ok(response::data(items))
}

/// `GET /api/v1/me/eco-reports` — the authenticated user's own reports.
pub async fn list_my_reports(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Value>, AppError> {
    let rows: Vec<EcoRow> = sqlx::query_as(&format!(
        "SELECT {SELECT_COLS} FROM eco_reports \
         WHERE user_id = $1 ORDER BY created_at DESC LIMIT 50"
    ))
    .bind(auth.id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| row_json(r.0, r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8, r.9))
        .collect();
    Ok(response::data(items))
}
