//! Content reports — "Zgłoś" on other people's eco posts, comments and
//! profiles (Google Play UGC policy; DSA art. 16 notice mechanism, regulamin
//! pkt 7). Spec 2026-10-01.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::extractor::AuthUser,
    error::{AppError, FieldError},
    response,
    state::AppState,
};

/// Allowed `target_type` values.
pub const TARGET_TYPES: [&str; 3] = ["eco_post", "eco_comment", "user"];

/// Allowed `reason` values (mirrors the CHECK in migration 0013).
pub const REASONS: [&str; 8] = [
    "spam",
    "harassment",
    "hate",
    "sexual",
    "violence",
    "privacy",
    "illegal",
    "other",
];

/// Max length of the optional free-text note.
pub const NOTE_MAX: usize = 500;

/// Body for `POST /api/v1/reports`.
#[derive(Deserialize)]
pub struct CreateReportRequest {
    pub target_type: String,
    pub target_id: Uuid,
    pub reason: String,
    #[serde(default)]
    pub note: Option<String>,
}

fn invalid(field: &str, message: &str) -> FieldError {
    FieldError {
        field: field.into(),
        message: message.into(),
        code: "INVALID".into(),
    }
}

/// Pure input validation (unit-tested). Returns the trimmed, non-empty note.
pub fn validate(body: &CreateReportRequest) -> Result<Option<String>, AppError> {
    let mut errors = Vec::new();
    if !TARGET_TYPES.contains(&body.target_type.as_str()) {
        errors.push(invalid("target_type", "must be eco_post, eco_comment or user"));
    }
    if !REASONS.contains(&body.reason.as_str()) {
        errors.push(invalid("reason", "unknown reason"));
    }
    let note = body
        .note
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string);
    if let Some(n) = &note {
        crate::util::validate::check_max_len(&mut errors, "note", n, NOTE_MAX);
    }
    if errors.is_empty() {
        Ok(note)
    } else {
        Err(AppError::Validation(errors))
    }
}

/// Owner (author) of the reported target, or `None` when it does not exist.
async fn target_owner(
    state: &AppState,
    target_type: &str,
    target_id: Uuid,
) -> Result<Option<Uuid>, AppError> {
    let sql = match target_type {
        "eco_post" => "SELECT user_id FROM eco_reports WHERE id = $1",
        "eco_comment" => "SELECT user_id FROM eco_comments WHERE id = $1",
        _ => "SELECT id FROM users WHERE id = $1 AND deleted_at IS NULL",
    };
    sqlx::query_scalar(sql)
        .bind(target_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::internal)
}

/// `POST /api/v1/reports` — report someone else's content.
///
/// 201 `{ id, created: true }` for a new report; 200 `{ id, created: false }`
/// when the caller already reported this target (idempotent). 404 unknown
/// target; 422 bad input or reporting one's own content; 429 over quota.
/// The moderation mailbox is notified in the background (no report content
/// in the mail); a mail failure never affects the stored report.
pub async fn create_report(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateReportRequest>,
) -> Result<Response, AppError> {
    let note = validate(&body)?;

    crate::util::ratelimit::check_content_report_quota(auth.id)
        .map_err(|_| AppError::RateLimited)?;

    let owner = target_owner(&state, &body.target_type, body.target_id)
        .await?
        .ok_or(AppError::NotFound)?;
    if owner == auth.id {
        return Err(AppError::Validation(vec![invalid(
            "target_id",
            "cannot report your own content",
        )]));
    }

    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO content_reports (reporter_id, target_type, target_id, reason, note) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (reporter_id, target_type, target_id) DO NOTHING \
         RETURNING id",
    )
    .bind(auth.id)
    .bind(&body.target_type)
    .bind(body.target_id)
    .bind(&body.reason)
    .bind(note)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::internal)?;

    let (id, created) = match inserted {
        Some(id) => (id, true),
        None => {
            let id: Uuid = sqlx::query_scalar(
                "SELECT id FROM content_reports \
                 WHERE reporter_id = $1 AND target_type = $2 AND target_id = $3",
            )
            .bind(auth.id)
            .bind(&body.target_type)
            .bind(body.target_id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;
            (id, false)
        }
    };

    if created {
        notify_admin(&state, id, &body.target_type);
    }

    let status = if created { StatusCode::CREATED } else { StatusCode::OK };
    Ok((
        status,
        response::data(serde_json::json!({ "id": id, "created": created })),
    )
        .into_response())
}

/// Fire-and-forget moderation mail (bounded by SMTP + outer timeout).
fn notify_admin(state: &AppState, report_id: Uuid, target_type: &str) {
    let (Some(mail), Some(to)) = (state.config.mail.clone(), state.config.admin_email.clone())
    else {
        tracing::warn!(report = %report_id, "content report stored; admin mail not configured");
        return;
    };
    let target_type = target_type.to_string();
    tokio::spawn(async move {
        let send = crate::mail::send_report_notification(&mail, &to, report_id, &target_type);
        match tokio::time::timeout(std::time::Duration::from_secs(30), send).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!(report = %report_id, error = %e, "report mail failed"),
            Err(_) => tracing::warn!(report = %report_id, "report mail timed out"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(target_type: &str, reason: &str, note: Option<&str>) -> CreateReportRequest {
        CreateReportRequest {
            target_type: target_type.into(),
            target_id: Uuid::nil(),
            reason: reason.into(),
            note: note.map(str::to_string),
        }
    }

    #[test]
    fn accepts_valid_input_and_trims_note() {
        let note = validate(&req("eco_post", "spam", Some("  hej  "))).unwrap();
        assert_eq!(note.as_deref(), Some("hej"));
        assert_eq!(validate(&req("user", "other", Some("   "))).unwrap(), None);
        assert_eq!(validate(&req("eco_comment", "hate", None)).unwrap(), None);
    }

    #[test]
    fn rejects_bad_target_reason_and_long_note() {
        assert!(validate(&req("walk", "spam", None)).is_err());
        assert!(validate(&req("eco_post", "boring", None)).is_err());
        let long = "x".repeat(NOTE_MAX + 1);
        assert!(validate(&req("eco_post", "spam", Some(&long))).is_err());
        let max = "ż".repeat(NOTE_MAX);
        assert!(validate(&req("eco_post", "spam", Some(&max))).is_ok());
    }
}
