//! Small server-side input-validation helpers shared across route handlers.
//!
//! Rationale (security audit 2026-07-08): text fields were stored without upper
//! length bounds, and URL fields (`avatar_url`, `photo_*_url`) accepted any
//! string — including `javascript:` schemes that a client could later render as
//! a link. These helpers build the same `FieldError` shape the handlers already
//! use, so callers just collect them into an `AppError::Validation`.

use crate::error::FieldError;

/// Build a `FieldError` with an `INVALID_LENGTH` code.
fn too_long(field: &str, max: usize) -> FieldError {
    FieldError {
        field: field.into(),
        message: format!("must be at most {max} characters"),
        code: "INVALID_LENGTH".into(),
    }
}

/// If `value` (counted in Unicode scalar values) exceeds `max`, push a
/// length error for `field` onto `errors`.
pub fn check_max_len(errors: &mut Vec<FieldError>, field: &str, value: &str, max: usize) {
    if value.chars().count() > max {
        errors.push(too_long(field, max));
    }
}

/// True only for `http://` / `https://` URLs. Blocks `javascript:`, `data:`,
/// and other schemes that are unsafe to render as a link/image `src`.
pub fn is_safe_url(value: &str) -> bool {
    let v = value.trim();
    v.starts_with("https://") || v.starts_with("http://")
}

/// If `value` is present and non-empty but not a safe http(s) URL, push an
/// error for `field`. Empty/blank is treated as "not provided" (no error).
pub fn check_optional_url(errors: &mut Vec<FieldError>, field: &str, value: Option<&str>) {
    if let Some(v) = value {
        let t = v.trim();
        if !t.is_empty() && !is_safe_url(t) {
            errors.push(FieldError {
                field: field.into(),
                message: "must be a http(s) URL".into(),
                code: "INVALID_URL".into(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_url_schemes() {
        assert!(is_safe_url("https://example.com/a.jpg"));
        assert!(is_safe_url("http://example.com"));
        assert!(!is_safe_url("javascript:alert(1)"));
        assert!(!is_safe_url("data:text/html;base64,x"));
        assert!(!is_safe_url("ftp://x"));
        assert!(!is_safe_url("  javascript:alert(1)"));
    }

    #[test]
    fn max_len_counts_unicode_scalars() {
        let mut e = Vec::new();
        check_max_len(&mut e, "f", "abc", 3);
        assert!(e.is_empty());
        check_max_len(&mut e, "f", "abcd", 3);
        assert_eq!(e.len(), 1);
        // 3 multi-byte chars, limit 3 → ok (counts chars, not bytes)
        let mut e2 = Vec::new();
        check_max_len(&mut e2, "f", "ąćę", 3);
        assert!(e2.is_empty());
    }

    #[test]
    fn optional_url_empty_is_ok() {
        let mut e = Vec::new();
        check_optional_url(&mut e, "u", None);
        check_optional_url(&mut e, "u", Some(""));
        check_optional_url(&mut e, "u", Some("   "));
        assert!(e.is_empty());
        check_optional_url(&mut e, "u", Some("javascript:x"));
        assert_eq!(e.len(), 1);
    }
}
