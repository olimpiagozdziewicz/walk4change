//! Bug reports — „Zgłoś błąd” in the app: category, description, device
//! metadata and an optional walk trace (spec
//! seasteps/spec/2026-10-07-zapis-spaceru-do-zgloszenia-bledu.md).

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

/// Allowed `category` values (mirrors the CHECK in migration 0016).
pub const CATEGORIES: [&str; 5] = ["walk", "steps", "map", "login", "other"];

/// Max length (chars) of the description after trimming.
pub const DESCRIPTION_MAX: usize = 2000;
/// Max lengths (chars) of the optional metadata fields.
pub const APP_VERSION_MAX: usize = 40;
pub const DEVICE_MAX: usize = 80;
pub const PLATFORM_MAX: usize = 20;
pub const SCREEN_MAX: usize = 60;
/// Max size of the serialized `trace` object, in bytes.
pub const TRACE_MAX_BYTES: usize = 1_000_000;

/// HTTP body limit for this route only (global default is 64 KiB): the trace
/// may be up to 1 MB of JSON, plus the rest of the body and some headroom.
pub const BODY_LIMIT_BYTES: usize = 1_536_000;

// Compile-time guard: the route limit must fit a max trace plus a max
// description (≤ 4 bytes/char in UTF-8) and the metadata fields.
const _: () = assert!(BODY_LIMIT_BYTES > TRACE_MAX_BYTES + 4 * DESCRIPTION_MAX + 4096);

/// Body for `POST /api/v1/bug-reports`.
#[derive(Deserialize)]
pub struct CreateBugReportRequest {
    pub category: String,
    pub description: String,
    #[serde(default)]
    pub app_version: Option<String>,
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub screen: Option<String>,
    #[serde(default)]
    pub trace: Option<serde_json::Value>,
}

/// Normalised, validated report ready to insert.
#[derive(Debug, PartialEq)]
pub struct ValidBugReport {
    pub description: String,
    pub app_version: Option<String>,
    pub device: Option<String>,
    pub platform: Option<String>,
    pub screen: Option<String>,
    /// Serialized trace object (bound as text, cast to jsonb in SQL).
    pub trace: Option<String>,
}

fn invalid(field: &str, message: &str) -> FieldError {
    FieldError {
        field: field.into(),
        message: message.into(),
        code: "INVALID".into(),
    }
}

/// Trim; blank → `None`.
fn clean(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// Pure input validation (unit-tested).
pub fn validate(body: &CreateBugReportRequest) -> Result<ValidBugReport, AppError> {
    let mut errors = Vec::new();
    if !CATEGORIES.contains(&body.category.as_str()) {
        errors.push(invalid("category", "must be walk, steps, map, login or other"));
    }

    let description = body.description.trim().to_string();
    if description.is_empty() {
        errors.push(invalid("description", "must not be empty"));
    } else {
        crate::util::validate::check_max_len(
            &mut errors,
            "description",
            &description,
            DESCRIPTION_MAX,
        );
    }

    let app_version = clean(body.app_version.as_deref());
    let device = clean(body.device.as_deref());
    let platform = clean(body.platform.as_deref());
    let screen = clean(body.screen.as_deref());
    for (field, value, max) in [
        ("app_version", &app_version, APP_VERSION_MAX),
        ("device", &device, DEVICE_MAX),
        ("platform", &platform, PLATFORM_MAX),
        ("screen", &screen, SCREEN_MAX),
    ] {
        if let Some(v) = value {
            crate::util::validate::check_max_len(&mut errors, field, v, max);
        }
    }

    // `null` deserializes to `None` — treated as "no trace".
    let trace = match &body.trace {
        None => None,
        Some(t) if !t.is_object() => {
            errors.push(invalid("trace", "must be a JSON object"));
            None
        }
        Some(t) => {
            let s = t.to_string();
            if s.len() > TRACE_MAX_BYTES {
                errors.push(FieldError {
                    field: "trace".into(),
                    message: format!("must be at most {TRACE_MAX_BYTES} bytes"),
                    code: "INVALID_LENGTH".into(),
                });
                None
            } else {
                Some(s)
            }
        }
    };

    if errors.is_empty() {
        Ok(ValidBugReport {
            description,
            app_version,
            device,
            platform,
            screen,
            trace,
        })
    } else {
        Err(AppError::Validation(errors))
    }
}

/// `POST /api/v1/bug-reports` — report an app bug.
///
/// 201 `{ id }`. 401 no/invalid token; 413 body over the route limit;
/// 422 bad input; 429 over the per-account quota (5/h). The admin mailbox is
/// notified in the background (id + category only, no description or trace);
/// a mail failure never affects the stored report.
pub async fn create_bug_report(
    auth: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateBugReportRequest>,
) -> Result<Response, AppError> {
    let report = validate(&body)?;

    crate::util::ratelimit::check_bug_report_quota(auth.id)
        .map_err(|_| AppError::RateLimited)?;

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO bug_reports \
            (user_id, category, description, app_version, device, platform, screen, trace) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb) \
         RETURNING id",
    )
    .bind(auth.id)
    .bind(&body.category)
    .bind(&report.description)
    .bind(&report.app_version)
    .bind(&report.device)
    .bind(&report.platform)
    .bind(&report.screen)
    .bind(&report.trace)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::internal)?;

    notify_admin(&state, id, &body.category);

    Ok((
        StatusCode::CREATED,
        response::data(serde_json::json!({ "id": id })),
    )
        .into_response())
}

/// Fire-and-forget admin mail (bounded by SMTP + outer timeout).
fn notify_admin(state: &AppState, report_id: Uuid, category: &str) {
    let (Some(mail), Some(to)) = (state.config.mail.clone(), state.config.admin_email.clone())
    else {
        tracing::warn!(report = %report_id, "bug report stored; admin mail not configured");
        return;
    };
    let category = category.to_string();
    tokio::spawn(async move {
        let send = crate::mail::send_bug_report_notification(&mail, &to, report_id, &category);
        match tokio::time::timeout(std::time::Duration::from_secs(30), send).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => tracing::warn!(report = %report_id, error = %e, "bug report mail failed"),
            Err(_) => tracing::warn!(report = %report_id, "bug report mail timed out"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn req(category: &str, description: &str) -> CreateBugReportRequest {
        CreateBugReportRequest {
            category: category.into(),
            description: description.into(),
            app_version: None,
            device: None,
            platform: None,
            screen: None,
            trace: None,
        }
    }

    fn fields(err: AppError) -> Vec<String> {
        match err {
            AppError::Validation(f) => f.into_iter().map(|e| e.field).collect(),
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn accepts_valid_input_and_normalises() {
        let mut r = req("walk", "  Spacer się zatrzymał  ");
        r.app_version = Some(" 1.4.2 ".into());
        r.device = Some("   ".into());
        r.platform = Some("android".into());
        r.trace = Some(json!({ "points": [[54.5, 18.5]], "steps": 1200 }));
        let v = validate(&r).unwrap();
        assert_eq!(v.description, "Spacer się zatrzymał");
        assert_eq!(v.app_version.as_deref(), Some("1.4.2"));
        assert_eq!(v.device, None);
        assert_eq!(v.platform.as_deref(), Some("android"));
        assert_eq!(v.screen, None);
        let t: serde_json::Value = serde_json::from_str(v.trace.as_deref().unwrap()).unwrap();
        assert_eq!(t["steps"], 1200);

        for c in CATEGORIES {
            assert!(validate(&req(c, "x")).is_ok(), "{c}");
        }
    }

    #[test]
    fn rejects_unknown_category() {
        assert_eq!(fields(validate(&req("crash", "x")).unwrap_err()), ["category"]);
        assert!(validate(&req("", "x")).is_err());
        assert!(validate(&req("Walk", "x")).is_err());
    }

    #[test]
    fn rejects_empty_and_too_long_description_counting_chars() {
        assert_eq!(fields(validate(&req("map", "")).unwrap_err()), ["description"]);
        assert_eq!(fields(validate(&req("map", "  \n\t ")).unwrap_err()), ["description"]);
        // 2000 Polish chars = 4000 bytes → still OK (chars, not bytes)
        let max = "ż".repeat(DESCRIPTION_MAX);
        assert!(validate(&req("map", &max)).is_ok());
        let too_long = "ą".repeat(DESCRIPTION_MAX + 1);
        assert_eq!(fields(validate(&req("map", &too_long)).unwrap_err()), ["description"]);
        // trimming happens before the length check
        let padded = format!("   {max}   ");
        assert!(validate(&req("map", &padded)).is_ok());
    }

    #[test]
    fn rejects_too_long_metadata() {
        let mut r = req("steps", "x");
        r.screen = Some("ś".repeat(SCREEN_MAX + 1));
        r.platform = Some("p".repeat(PLATFORM_MAX + 1));
        let mut f = fields(validate(&r).unwrap_err());
        f.sort();
        assert_eq!(f, ["platform", "screen"]);
    }

    #[test]
    fn rejects_oversized_trace() {
        let mut r = req("walk", "x");
        // {"d":"…"} → 8 bytes of overhead
        r.trace = Some(json!({ "d": "a".repeat(TRACE_MAX_BYTES) }));
        assert_eq!(fields(validate(&r).unwrap_err()), ["trace"]);
        r.trace = Some(json!({ "d": "a".repeat(TRACE_MAX_BYTES - 8) }));
        assert!(validate(&r).is_ok());
        r.trace = Some(json!({ "d": "a".repeat(TRACE_MAX_BYTES - 7) }));
        assert!(validate(&r).is_err());
    }

    #[test]
    fn rejects_non_object_trace() {
        for t in [json!([1, 2, 3]), json!("trace"), json!(42), json!(true)] {
            let mut r = req("walk", "x");
            r.trace = Some(t);
            assert_eq!(fields(validate(&r).unwrap_err()), ["trace"]);
        }
    }

    #[test]
    fn json_null_trace_means_no_trace() {
        let r: CreateBugReportRequest =
            serde_json::from_str(r#"{"category":"other","description":"x","trace":null}"#)
                .unwrap();
        assert_eq!(validate(&r).unwrap().trace, None);
    }
}
