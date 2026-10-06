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
    repo::{magic as magic_repo, user as user_repo, verify as verify_repo},
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
    /// Sign-up consent to the terms + privacy policy (RODO, spec 2026-07-13).
    /// Defaults to false so older clients get a clear TERMS_REQUIRED error.
    #[serde(default)]
    pub accepted_terms: bool,
}

/// Push a TERMS_REQUIRED field error when sign-up consent is missing.
fn require_terms(errors: &mut Vec<FieldError>, accepted: bool) {
    if !accepted {
        errors.push(FieldError {
            field: "accepted_terms".into(),
            message: "terms and privacy policy must be accepted".into(),
            code: "TERMS_REQUIRED".into(),
        });
    }
}

/// Mint a verification token and e-mail the confirmation link.
/// Errors bubble up; callers on the registration path treat this as
/// best-effort (mail outage must not break sign-up).
async fn send_verification_mail(
    state: &AppState,
    user_id: Uuid,
    email: &str,
) -> Result<(), AppError> {
    let mail_cfg = state
        .config
        .mail
        .as_ref()
        .ok_or_else(|| AppError::internal("verification email is not configured"))?;

    let token = random_string(48);
    verify_repo::create_token(&state.pool, &token, user_id).await?;
    let link = format!(
        "{}/auth/verify-email?token={}",
        state.config.app_url.trim_end_matches('/'),
        token
    );
    mail::send_verification_email(mail_cfg, email, &link).await
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

    // Normalize the email the same way supabase_exchange does, so a
    // single address can't spawn duplicate accounts differing only by case.
    let email = body.email.trim().to_lowercase();
    let display_name = body.display_name.trim();

    if !email.contains('@') {
        errors.push(FieldError {
            field: "email".into(),
            message: "must contain @".into(),
            code: "INVALID_EMAIL".into(),
        });
    }
    crate::util::validate::check_max_len(&mut errors, "email", &email, 254);
    if body.password.len() < 8 || body.password.len() > 128 {
        errors.push(FieldError {
            field: "password".into(),
            message: "must be between 8 and 128 characters".into(),
            code: "INVALID_LENGTH".into(),
        });
    }
    if display_name.is_empty() {
        errors.push(FieldError {
            field: "display_name".into(),
            message: "must not be empty".into(),
            code: "REQUIRED".into(),
        });
    }
    crate::util::validate::check_max_len(&mut errors, "display_name", display_name, 80);
    require_terms(&mut errors, body.accepted_terms);

    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }

    let id = Uuid::new_v4();
    let password_hash = password::hash(&state.config, &body.password)?;
    user_repo::create(&state.pool, id, &email, &password_hash, display_name, true).await?;

    // Best-effort verification mail — a mail outage must not break sign-up
    // (the user can re-request from their profile).
    if let Err(e) = send_verification_mail(&state, id, &email).await {
        tracing::warn!(user = %id, error = %e, "verification mail failed at registration");
    }

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
            // Konta sprzed flow zgód: formularz logowania wyświetla klauzulę
            // akceptacji — odnotuj zgodę, jeśli jeszcze nie zapisana.
            user_repo::record_terms_if_missing(&state.pool, id).await?;
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

// `POST /api/v1/auth/magic/request` (SMTP magic link) zdjęty z routera
// 2026-10-06 (audyt, Low): frontend loguje przez Supabase OTP
// (`/auth/supabase`), a stary endpoint zakładał konta i wysyłał maile na
// dowolne adresy. `/auth/magic/verify` zostaje dla tokenów już wysłanych.

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
    // Consuming a mailed one-time token proves mailbox ownership.
    claim_email(&state, user_id).await?;
    user_repo::record_terms_if_missing(&state.pool, user_id).await?;
    let token = jwt::encode(&state.config, user_id)?;
    let profile = user_repo::get_profile(&state.pool, user_id).await?;
    Ok(Json(json!({ "token": token, "data": profile })))
}

/// `POST /api/v1/auth/verify-email/request`
///
/// Send (or re-send) the e-mail verification link for the authenticated user.
/// Idempotent when already verified (204 without sending). Rate-limited
/// 3/min per account. 204 on success.
pub async fn verify_email_request(
    auth: AuthUser,
    State(state): State<AppState>,
) -> Result<StatusCode, AppError> {
    crate::util::ratelimit::check_verify_email_quota(auth.id)
        .map_err(|_| AppError::RateLimited)?;

    let profile = user_repo::get_profile(&state.pool, auth.id).await?;
    if profile.email_verified {
        return Ok(StatusCode::NO_CONTENT);
    }

    send_verification_mail(&state, auth.id, &profile.email).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct VerifyEmailConfirm {
    pub token: String,
}

/// `POST /api/v1/auth/verify-email/confirm`
///
/// Consume a verification token and mark the e-mail verified. Unlike
/// magic-link, this NEVER mints a session (a verification mail must not be a
/// login credential). Invalid/expired token → 401.
pub async fn verify_email_confirm(
    State(state): State<AppState>,
    Json(body): Json<VerifyEmailConfirm>,
) -> Result<Json<Value>, AppError> {
    let user_id = verify_repo::consume_token(&state.pool, body.token.trim()).await?;
    // Świadomie BEZ podmiany hasła (inaczej niż magic/supabase, audyt H3):
    // ten link dostaje każdy po rejestracji hasłem — unieważnienie hasła
    // wylogowałoby na stałe każdego normalnego użytkownika, a link nie loguje.
    user_repo::set_email_verified(&state.pool, user_id).await?;
    Ok(Json(json!({ "data": { "verified": true } })))
}

#[derive(Deserialize)]
pub struct SupabaseExchange {
    pub access_token: String,
    /// Consent to terms + privacy (clause under the magic-link form).
    /// Required only when the exchange CREATES a new account.
    #[serde(default)]
    pub accepted_terms: bool,
}

#[derive(Deserialize)]
struct SupabaseUser {
    email: Option<String>,
    /// Supabase ustawia to po potwierdzeniu skrzynki; bez tego token nie
    /// dowodzi kontroli nad adresem (audyt 2026-10-06, Low).
    #[serde(default)]
    email_confirmed_at: Option<String>,
}

/// Znormalizowany e-mail z usera Supabase — tylko gdy adres jest
/// POTWIERDZONY (`email_confirmed_at`) i wygląda na e-mail; inaczej `None`.
fn confirmed_supabase_email(user: SupabaseUser) -> Option<String> {
    let confirmed = user
        .email_confirmed_at
        .as_deref()
        .is_some_and(|c| !c.trim().is_empty());
    if !confirmed {
        return None;
    }
    user.email
        .map(|e| e.trim().to_lowercase())
        .filter(|e| e.contains('@'))
}

/// Logowanie dowiodło kontroli nad skrzynką: przy niezweryfikowanym koncie
/// zweryfikuj e-mail i unieważnij hasło (ktoś mógł zarejestrować cudzy adres
/// z własnym hasłem) — patrz [`user_repo::claim_unverified_email`].
async fn claim_email(state: &AppState, user_id: Uuid) -> Result<(), AppError> {
    if user_repo::is_email_verified(&state.pool, user_id).await? {
        return Ok(());
    }
    let unusable = password::hash(&state.config, &random_string(40))?;
    user_repo::claim_unverified_email(&state.pool, user_id, &unusable).await
}

/// `POST /api/v1/auth/supabase`
///
/// Bridge for Supabase-Auth magic links: validate a Supabase access token by
/// calling Supabase's `/auth/v1/user`, then find-or-create the matching local
/// user and return THIS service's JWT + profile (so all app data keeps using
/// the backend's own auth). Invalid token → 401.
pub async fn supabase_exchange(
    State(state): State<AppState>,
    Json(body): Json<SupabaseExchange>,
) -> Result<Json<Value>, AppError> {
    let url = state
        .config
        .supabase_url
        .as_ref()
        .ok_or_else(|| AppError::internal("supabase auth is not configured"))?;
    let anon = state
        .config
        .supabase_anon_key
        .as_ref()
        .ok_or_else(|| AppError::internal("supabase auth is not configured"))?;

    let resp = crate::util::http::client()
        .get(format!("{url}/auth/v1/user"))
        .header("apikey", anon)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", body.access_token.trim()))
        .send()
        .await
        .map_err(AppError::internal)?;

    if !resp.status().is_success() {
        return Err(AppError::Unauthorized);
    }

    let user: SupabaseUser = resp.json().await.map_err(AppError::internal)?;
    let email = confirmed_supabase_email(user).ok_or(AppError::Unauthorized)?;

    let user_id = match user_repo::find_by_email(&state.pool, &email).await? {
        Some(u) => u.id,
        None => {
            // Creating an account requires sign-up consent (RODO, spec 2026-07-13).
            let mut errors = Vec::new();
            require_terms(&mut errors, body.accepted_terms);
            if !errors.is_empty() {
                return Err(AppError::Validation(errors));
            }
            let id = Uuid::new_v4();
            let hash = password::hash(&state.config, &random_string(40))?;
            let display = email.split('@').next().unwrap_or("walker");
            user_repo::create(&state.pool, id, &email, &hash, display, true).await?;
            id
        }
    };

    // Supabase already validated the mailbox via its own magic link.
    claim_email(&state, user_id).await?;
    user_repo::record_terms_if_missing(&state.pool, user_id).await?;

    let token = jwt::encode(&state.config, user_id)?;
    let profile = user_repo::get_profile(&state.pool, user_id).await?;
    Ok(Json(json!({ "token": token, "data": profile })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn su(email: Option<&str>, confirmed: Option<&str>) -> SupabaseUser {
        SupabaseUser {
            email: email.map(str::to_owned),
            email_confirmed_at: confirmed.map(str::to_owned),
        }
    }

    #[test]
    fn supabase_email_requires_confirmation() {
        assert_eq!(confirmed_supabase_email(su(Some("a@b.pl"), None)), None);
        assert_eq!(confirmed_supabase_email(su(Some("a@b.pl"), Some(""))), None);
    }

    #[test]
    fn supabase_email_confirmed_is_normalized() {
        assert_eq!(
            confirmed_supabase_email(su(Some(" A@B.pl "), Some("2026-10-06T10:00:00Z"))),
            Some("a@b.pl".to_owned())
        );
    }

    #[test]
    fn supabase_email_must_look_like_email() {
        assert_eq!(confirmed_supabase_email(su(Some("nope"), Some("2026-10-06T10:00:00Z"))), None);
        assert_eq!(confirmed_supabase_email(su(None, Some("2026-10-06T10:00:00Z"))), None);
    }

    #[test]
    fn supabase_user_json_without_confirmation_parses() {
        let u: SupabaseUser = serde_json::from_str(r#"{"email":"a@b.pl","email_confirmed_at":null}"#).unwrap();
        assert_eq!(confirmed_supabase_email(u), None);
    }
}
