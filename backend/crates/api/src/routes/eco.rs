use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
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

/// Points credited to `user_totals` per eco action — matches the copy the
/// frontend has always shown ("+15 pkt za czujność" / "+25 pkt").
const POINTS_REPORT: i32 = 15;
const POINTS_CLEANUP: i32 = 25;

/// Max point-earning eco reports per user per day (anti-spam; further
/// submissions still save, they just stop paying out).
const MAX_PAID_REPORTS_PER_DAY: i64 = 10;

/// `POST /api/v1/eco/reports` — create an eco report for the authenticated user.
///
/// Also credits points (report +15 / cleanup +25) to `user_totals`, capped at
/// [`MAX_PAID_REPORTS_PER_DAY`] paid submissions per day.
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
    let points = if kind == "cleanup" { POINTS_CLEANUP } else { POINTS_REPORT };

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

    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;

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
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    // Count today's submissions INCLUDING the row just inserted.
    let today_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM eco_reports \
         WHERE user_id = $1 AND created_at >= date_trunc('day', now())",
    )
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::internal)?;

    if today_count <= MAX_PAID_REPORTS_PER_DAY {
        sqlx::query(
            "INSERT INTO user_totals (user_id, total_points) VALUES ($1, $2) \
             ON CONFLICT (user_id) DO UPDATE \
             SET total_points = user_totals.total_points + EXCLUDED.total_points, \
                 updated_at = now()",
        )
        .bind(auth.id)
        .bind(rust_decimal::Decimal::from(points))
        .execute(&mut *tx)
        .await
        .map_err(AppError::internal)?;
    }

    tx.commit().await.map_err(AppError::internal)?;

    Ok(response::data(row_json(
        row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8, row.9,
    )))
}

/// One feed row: report columns + author + like/comment counters.
type FeedRow = (
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
    String,
    i64,
    i64,
    bool,
);

/// `GET /api/v1/eco/reports` — recent reports across all users (community feed):
/// author `display_name`, `like_count`, `comment_count` and `liked_by_me`.
pub async fn list_reports(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Value>, AppError> {
    let rows: Vec<FeedRow> = sqlx::query_as(
        "SELECT e.id, e.kind, e.category, e.description, e.location, e.status, \
                e.photo_url, e.photo_before_url, e.photo_after_url, e.created_at, \
                u.display_name, \
                (SELECT count(*) FROM eco_likes l WHERE l.report_id = e.id) AS like_count, \
                (SELECT count(*) FROM eco_comments c WHERE c.report_id = e.id) AS comment_count, \
                EXISTS(SELECT 1 FROM eco_likes l2 WHERE l2.report_id = e.id AND l2.user_id = $1) AS liked_by_me \
         FROM eco_reports e \
         JOIN users u ON u.id = e.user_id \
         ORDER BY e.created_at DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;

    let items: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            let mut v = row_json(r.0, r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8, r.9);
            v["author"] = Value::String(r.10);
            v["like_count"] = Value::from(r.11);
            v["comment_count"] = Value::from(r.12);
            v["liked_by_me"] = Value::from(r.13);
            v
        })
        .collect();
    Ok(response::data(items))
}

/// `POST /api/v1/eco/reports/:id/like` — toggle a like on a feed entry.
///
/// Returns `{ liked, like_count }` after the toggle. 404 for unknown report.
pub async fn toggle_like(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(report_id): Path<uuid::Uuid>,
) -> Result<Json<Value>, AppError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM eco_reports WHERE id = $1)")
        .bind(report_id)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
    if !exists {
        return Err(AppError::NotFound);
    }

    let deleted = sqlx::query("DELETE FROM eco_likes WHERE report_id = $1 AND user_id = $2")
        .bind(report_id)
        .bind(auth.id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?
        .rows_affected();

    let liked = if deleted == 0 {
        sqlx::query(
            "INSERT INTO eco_likes (report_id, user_id) VALUES ($1, $2) \
             ON CONFLICT DO NOTHING",
        )
        .bind(report_id)
        .bind(auth.id)
        .execute(&state.pool)
        .await
        .map_err(AppError::internal)?;
        true
    } else {
        false
    };

    let like_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM eco_likes WHERE report_id = $1")
            .bind(report_id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::internal)?;

    Ok(response::data(
        serde_json::json!({ "liked": liked, "like_count": like_count }),
    ))
}

/// One comment row with its author's display name.
type CommentRow = (uuid::Uuid, uuid::Uuid, String, chrono::DateTime<chrono::Utc>, String);

fn comment_json(r: CommentRow) -> Value {
    serde_json::json!({
        "id": r.0,
        "user_id": r.1,
        "body": r.2,
        "created_at": r.3,
        "author": r.4,
    })
}

/// `GET /api/v1/eco/reports/:id/comments` — comments for a feed entry, oldest first.
pub async fn list_comments(
    _auth: AuthUser,
    State(state): State<AppState>,
    Path(report_id): Path<uuid::Uuid>,
) -> Result<Json<Value>, AppError> {
    let rows: Vec<CommentRow> = sqlx::query_as(
        "SELECT c.id, c.user_id, c.body, c.created_at, u.display_name \
         FROM eco_comments c \
         JOIN users u ON u.id = c.user_id \
         WHERE c.report_id = $1 \
         ORDER BY c.created_at \
         LIMIT 100",
    )
    .bind(report_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::internal)?;

    Ok(response::data(rows.into_iter().map(comment_json).collect::<Vec<_>>()))
}

/// Body for `POST /api/v1/eco/reports/:id/comments`.
#[derive(Deserialize)]
pub struct CreateCommentRequest {
    pub body: String,
}

/// `POST /api/v1/eco/reports/:id/comments` — add a comment (1..=500 chars).
pub async fn create_comment(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(report_id): Path<uuid::Uuid>,
    Json(body): Json<CreateCommentRequest>,
) -> Result<Response, AppError> {
    let text = body.body.trim();
    if text.is_empty() || text.chars().count() > 500 {
        return Err(AppError::Validation(vec![FieldError {
            field: "body".into(),
            message: "comment must be 1..=500 characters".into(),
            code: "INVALID_LENGTH".into(),
        }]));
    }

    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM eco_reports WHERE id = $1)")
        .bind(report_id)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::internal)?;
    if !exists {
        return Err(AppError::NotFound);
    }

    let id = uuid::Uuid::new_v4();
    let row: CommentRow = sqlx::query_as(
        "WITH ins AS ( \
             INSERT INTO eco_comments (id, report_id, user_id, body) \
             VALUES ($1, $2, $3, $4) \
             RETURNING id, user_id, body, created_at \
         ) \
         SELECT ins.id, ins.user_id, ins.body, ins.created_at, u.display_name \
         FROM ins JOIN users u ON u.id = ins.user_id",
    )
    .bind(id)
    .bind(report_id)
    .bind(auth.id)
    .bind(text)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::internal)?;

    Ok((StatusCode::CREATED, response::data(comment_json(row))).into_response())
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
