use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::extractor::AuthUser,
    error::AppError,
    repo::walk as walk_repo,
    response,
    state::AppState,
};

/// Query parameters for `GET /api/v1/walks/:id/track`.
#[derive(Deserialize)]
pub struct TrackQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    1000
}

/// Optional body for `POST /api/v1/walks` ("spaceruję — dołącz" opt-in).
#[derive(Deserialize, Default)]
pub struct StartWalkBody {
    #[serde(default)]
    pub is_open: bool,
    #[serde(default)]
    pub open_note: Option<String>,
}

/// `POST /api/v1/walks`
///
/// Start a new walk session. The authenticated user becomes the host and is
/// automatically added as the first participant. An optional JSON body may
/// set `is_open` (+ `open_note`) to list the walk publicly so others can join
/// (the body stays optional for backward compatibility with older clients).
///
/// Returns 201 with a `Location` header pointing to the new session and the
/// session data (including `join_code`) in the body.
pub async fn start_walk(
    auth: AuthUser,
    State(state): State<AppState>,
    body: Option<Json<StartWalkBody>>,
) -> Result<Response, AppError> {
    let StartWalkBody { is_open, open_note } = body.map(|Json(b)| b).unwrap_or_default();
    let session_id = Uuid::new_v4();
    let session = walk_repo::start(
        &state.pool,
        session_id,
        auth.id,
        is_open,
        open_note.as_deref(),
    )
    .await?;
    let location = format!("/api/v1/walks/{}", session.id);
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        response::data(session),
    )
        .into_response())
}

/// Body for `PATCH /api/v1/walks/:id` ("spaceruję — dołącz" toggle mid-walk).
#[derive(Deserialize)]
pub struct PatchWalkBody {
    pub is_open: bool,
    #[serde(default)]
    pub open_note: Option<String>,
}

/// `PATCH /api/v1/walks/:id`
///
/// Host-only: toggle the session's open visibility without stopping the walk.
/// Returns 204; 403 when not the host or the session is not active.
pub async fn patch_walk(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(body): Json<PatchWalkBody>,
) -> Result<StatusCode, AppError> {
    walk_repo::set_open(
        &state.pool,
        session_id,
        auth.id,
        body.is_open,
        body.open_note.as_deref(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/v1/walks/open`
///
/// List currently-active open walks ("spaceruję — dołącz"), newest first.
/// Any authenticated user can browse and join these via `POST /walks/:id/join`.
pub async fn open_walks(
    _auth: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Value>, AppError> {
    let walks = walk_repo::open_walks(&state.pool).await?;
    Ok(response::data(walks))
}

/// Query parameters for `GET /api/v1/me/walks`.
#[derive(Deserialize)]
pub struct MyWalksQuery {
    #[serde(default = "default_my_walks_limit")]
    pub limit: i64,
}

fn default_my_walks_limit() -> i64 {
    50
}

/// `GET /api/v1/me/walks`
///
/// The caller's finished walks, newest first (server-side walk history).
pub async fn my_walks(
    auth: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<MyWalksQuery>,
) -> Result<Json<Value>, AppError> {
    let walks = walk_repo::my_walks(&state.pool, auth.id, query.limit).await?;
    Ok(response::data(walks))
}

/// `POST /api/v1/walks/:id/join`
///
/// Join an active walk session. The actor must be a friend of the host.
///
/// Returns 200 on success. Returns 404 if the session is not found or not active,
/// 403 if not a friend, 409 if already joined.
pub async fn join_walk(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    walk_repo::join(&state.pool, session_id, auth.id).await?;
    Ok(response::data(serde_json::json!({})))
}

/// Body for `POST /api/v1/walks/join-by-code`.
#[derive(Deserialize)]
pub struct JoinByCodeRequest {
    pub code: String,
}

/// `POST /api/v1/walks/join-by-code`
///
/// Join an active walk by its short `join_code`, WITHOUT requiring friendship.
/// Powers the two-phone demo where strangers pair via a code. Returns the
/// session id (`{ "data": { "session_id": <uuid> } }`).
///
/// Returns 200 on success (idempotent if already joined), 404 if no active
/// session has that code.
pub async fn join_by_code(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<JoinByCodeRequest>,
) -> Result<Json<Value>, AppError> {
    let code = body.code.trim().to_uppercase();
    let session_id = walk_repo::join_by_code(&state.pool, &code, auth.id).await?;
    Ok(response::data(serde_json::json!({ "session_id": session_id })))
}

/// `POST /api/v1/walks/:id/leave`
///
/// Leave a walk session by setting `left_at = now()`.
/// Idempotent if already left. Returns 404 if never a member.
///
/// Returns 204 on success.
pub async fn leave_walk(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    walk_repo::leave(&state.pool, session_id, auth.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /api/v1/walks/:id/stop`
///
/// Stop an active walk session (host only). Marks the session as `finished`,
/// closes all open participants, and increments `user_totals.total_walks` for
/// every participant.
///
/// Returns 204 on success, 403 if not the host or session already finished.
pub async fn stop_walk(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    walk_repo::stop(&state.pool, session_id, auth.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/v1/walks/:id`
///
/// Fetch full walk detail: session metadata and participants list.
/// Member-only (any participant, including those who left).
///
/// Returns 200 with `{ data: { session, participants } }`.
pub async fn get_walk(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    let detail = walk_repo::get(&state.pool, session_id, auth.id).await?;
    Ok(response::data(detail))
}

/// `GET /api/v1/walks/:id/track`
///
/// Fetch all location pings for a session, ordered by `seq`.
/// Member-only. Accepts an optional `limit` query parameter (default 1000).
///
/// Returns 200 with `{ data: [PingPoint] }`.
pub async fn track_walk(
    auth: AuthUser,
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Query(query): Query<TrackQuery>,
) -> Result<Json<Value>, AppError> {
    let pings = walk_repo::track(&state.pool, session_id, auth.id, query.limit).await?;
    Ok(response::data(pings))
}
