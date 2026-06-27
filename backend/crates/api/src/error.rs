use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use serde_json::json;

/// A single field-level validation failure.
#[derive(Debug, Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
    pub code: String,
}

/// Central application error type.
/// Each variant maps to a specific HTTP status and error envelope.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Forbidden")]
    Forbidden,
    #[error("Not found")]
    NotFound,
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Validation failed")]
    Validation(Vec<FieldError>),
    #[error("Too many requests")]
    RateLimited,
    /// Use for unexpected errors. Detail is logged server-side; generic message is sent to client.
    #[error("Internal server error")]
    Internal(String),
}

impl AppError {
    /// Construct an `Internal` error, capturing the display form of any error/message.
    pub fn internal<E: std::fmt::Display>(e: E) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message, details) = match self {
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Unauthorized".to_string(),
                None,
            ),
            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "Forbidden".to_string(),
                None,
            ),
            AppError::NotFound => (
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                "Not found".to_string(),
                None,
            ),
            AppError::Conflict(msg) => (
                StatusCode::CONFLICT,
                "CONFLICT",
                msg,
                None,
            ),
            AppError::Validation(fields) => {
                let details = serde_json::to_value(&fields).ok();
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "VALIDATION_ERROR",
                    "Validation failed".to_string(),
                    details,
                )
            }
            AppError::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "RATE_LIMITED",
                "Too many requests — please slow down".to_string(),
                None,
            ),
            AppError::Internal(msg) => {
                tracing::error!(detail = %msg, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "An unexpected error occurred".to_string(),
                    None,
                )
            }
        };

        let body = match details {
            Some(d) => json!({ "error": { "code": code, "message": message, "details": d } }),
            None => json!({ "error": { "code": code, "message": message } }),
        };

        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn error_maps_to_status() {
        assert_eq!(
            AppError::Unauthorized.into_response().status(),
            axum::http::StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AppError::Forbidden.into_response().status(),
            axum::http::StatusCode::FORBIDDEN
        );
        assert_eq!(
            AppError::NotFound.into_response().status(),
            axum::http::StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn conflict_maps_to_409() {
        let resp = AppError::Conflict("already exists".into()).into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[test]
    fn validation_maps_to_422() {
        let fields = vec![FieldError {
            field: "email".into(),
            message: "invalid format".into(),
            code: "INVALID_EMAIL".into(),
        }];
        let resp = AppError::Validation(fields).into_response();
        assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn rate_limited_maps_to_429() {
        let resp = AppError::RateLimited.into_response();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn internal_maps_to_500() {
        let resp = AppError::Internal("boom".into()).into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
