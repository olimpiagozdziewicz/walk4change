use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use rand::{distributions::Alphanumeric, Rng};

use crate::{
    auth::{extractor::AuthUser, jwt, password},
    error::{AppError, FieldError},
    mail,
    repo::{magic as magic_repo, user as user_repo},
    state::AppState,
};

/// Generate a random alphanumeric string of length `n`.
fn random_string(n: usize) -> String {
    rand::thread_rng().sample_iter(Alphanumeric).take(n).map(char::from).collect()
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// `POST /api/v1/auth/register`
///
/// Creates a new user account. Returns 201 with a JWT and the user profile.
/// Validates email format, password length, and display_name presence.
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, HeaderMap, Json<Value>), AppError> {
    let mut errors: Vec<FieldError> = Vec::new();

    if !body.email.contains('@') {
        errors.push(FieldError {
            field: "email".into(),
            message: "must contain @".into(),
            code: "INVALID_EMAIL".into(),
        });
    }
    if body.password.len() < 8 || body.password.len() > 128 {
        errors.push(FieldError {
            field: "password".into(),
            message: "must be between 8 and 128 characters".into(),
            code: "INVALID_LENGTH".into(),
        });
    }
    if body.display_name.trim().is_empty() {
        errors.push(FieldError {
            field: "display_name".into(),
            message: "must not be empty".into(),
            code: "REQUIRED".into(),
        });
    }

    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }

    let id = Uuid::new_v4();
    let password_hash = password::hash(&state.config, &body.password)?;
    user_repo::create(&state.pool, id, &body.email, &password_hash, &body.display_name).await?;

    let profile = user_repo::get_profile(&state.pool, id).await?;
    let token = jwt::encode(&state.config, id)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::LOCATION,
        HeaderValue::from_static("/api/v1/me"),
    );

    Ok((StatusCode::CREATED, headers, Json(json!({ "token": token, "data": profile }))))
}

/// `POST /api/v1/auth/login`
///
/// Authenticates with email and password. Always runs `password::verify`
/// (using `DUMMY_HASH` when the email is unknown) for timing-safe enumeration defense.
/// Returns a uniform 401 on any failure.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<Value>, AppError> {
    let row = user_repo::find_by_email(&state.pool, &body.email).await?;

    let (hash, user_id) = match row {
        Some(r) => (r.password_hash, Some(r.id)),
        None => (password::DUMMY_HASH.to_owned(), None),
    };

    // Always run a full argon2 comparison to prevent timing-based enumeration.
    let valid = password::verify(&hash, &body.password);

    match (valid, user_id) {
        (true, Some(id)) => {
            let token = jwt::encode(&state.config, id)?;
            Ok(Json(json!({ "token": token })))
        }
        _ => Err(AppError::Unauthorized),
    }
}

/// `POST /api/v1/auth/logout`
///
/// Clears the `wc_session` cookie. Requires a valid auth token.
/// Returns 204 No Content.
pub async fn logout(_auth: AuthUser) -> (StatusCode, HeaderMap) {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_static(
            "wc_session=; Max-Age=0; Path=/; HttpOnly; SameSite=Strict",
        ),
    );
    (StatusCode::NO_CONTENT, headers)
}

#[derive(Deserialize)]
pub struct MagicRequest {
    pub email: String,
}

/// `POST /api/v1/auth/magic/request`
///
/// Passwordless login step 1: find-or-create a user for `email`, mint a one-time
/// token, and email a magic link (`APP_URL/auth/magic?token=…`). Always returns
/// 200 (does not reveal whether the account existed). 503 if SMTP isn't configured.
pub async fn magic_request(
    State(state): State<AppState>,
    Json(body): Json<MagicRequest>,
) -> Result<Json<Value>, AppError> {
    let email = body.email.trim().to_lowercase();
    if !email.contains('@') {
        return Err(AppError::Validation(vec![FieldError {
            field: "email".into(),
            message: "must contain @".into(),
            code: "INVALID_EMAIL".into(),
        }]));
    }

    let mail_cfg = state
        .config
        .mail
        .as_ref()
        .ok_or_else(|| AppError::internal("magic-link email is not configured"))?;

    // Find or create a passwordless user (random unusable password hash).
    let user_id = match user_repo::find_by_email(&state.pool, &email).await? {
        Some(u) => u.id,
        None => {
            let id = Uuid::new_v4();
            let hash = password::hash(&state.config, &random_string(40))?;
            let display = email.split('@').next().unwrap_or("walker");
            user_repo::create(&state.pool, id, &email, &hash, display).await?;
            id
        }
    };

    let token = random_string(48);
    magic_repo::create_token(&state.pool, &token, user_id).await?;

    let link = format!("{}/auth/magic?token={}", state.config.app_url.trim_end_matches('/'), token);
    mail::send_magic_link(mail_cfg, &email, &link).await?;

    Ok(Json(json!({ "data": { "sent": true } })))
}

#[derive(Deserialize)]
pub struct MagicVerify {
    pub token: String,
}

/// `POST /api/v1/auth/magic/verify`
///
/// Passwordless login step 2: consume the one-time token and return a JWT +
/// profile, exactly like register/login. Invalid/expired token → 401.
pub async fn magic_verify(
    State(state): State<AppState>,
    Json(body): Json<MagicVerify>,
) -> Result<Json<Value>, AppError> {
    let user_id = magic_repo::consume_token(&state.pool, body.token.trim()).await?;
    let token = jwt::encode(&state.config, user_id)?;
    let profile = user_repo::get_profile(&state.pool, user_id).await?;
    Ok(Json(json!({ "token": token, "data": profile })))
}
