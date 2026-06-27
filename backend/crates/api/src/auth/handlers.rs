use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    auth::{extractor::AuthUser, jwt, password},
    error::{AppError, FieldError},
    repo::user as user_repo,
    state::AppState,
};

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
