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

/// Prefiks publicznych URL-i Supabase Storage naszego projektu:
/// `{SUPABASE_URL}/storage/v1/object/public/` + opcjonalnie `{bucket}/`
/// (pusty `bucket` = dowolny bucket projektu).
pub fn storage_public_prefix(supabase_url: &str, bucket: &str) -> String {
    let base = format!(
        "{}/storage/v1/object/public/",
        supabase_url.trim().trim_end_matches('/')
    );
    if bucket.is_empty() {
        base
    } else {
        format!("{base}{bucket}/")
    }
}

/// Ścieżka obiektu za `prefix`, ale tylko „czysta”: niepuste segmenty
/// `[A-Za-z0-9._-]`, bez `.`/`..`, bez `?`, `#`, `%` (audyt 2026-10-06, M1 —
/// z dowolnego URL-a z `/eco-photos/` w środku dało się wyprowadzić ścieżkę
/// cudzego pliku, który potem kasował service key przy usuwaniu konta).
pub fn storage_object_path<'a>(url: &'a str, prefix: &str) -> Option<&'a str> {
    let path = url.trim().strip_prefix(prefix)?;
    let ok = !path.is_empty()
        && path.split('/').all(|seg| {
            !seg.is_empty()
                && seg != "."
                && seg != ".."
                && seg
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        });
    ok.then_some(path)
}

/// Jak [`check_optional_url`], ale URL musi wskazywać obiekt w NASZYM
/// Supabase Storage (`prefix` z [`storage_public_prefix`]). Bez skonfigurowanego
/// `SUPABASE_URL` (`prefix = None`) każdy niepusty URL jest odrzucany.
pub fn check_optional_storage_url(
    errors: &mut Vec<FieldError>,
    field: &str,
    value: Option<&str>,
    prefix: Option<&str>,
) {
    if let Some(v) = value {
        let t = v.trim();
        if t.is_empty() {
            return;
        }
        let ok = prefix.is_some_and(|p| storage_object_path(t, p).is_some());
        if !ok {
            errors.push(FieldError {
                field: field.into(),
                message: "must be an uploaded app photo URL".into(),
                code: "INVALID_URL".into(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SB: &str = "https://abc.supabase.co";

    #[test]
    fn storage_prefix_shapes() {
        assert_eq!(
            storage_public_prefix("https://abc.supabase.co/", "eco-photos"),
            "https://abc.supabase.co/storage/v1/object/public/eco-photos/"
        );
        assert_eq!(
            storage_public_prefix(SB, ""),
            "https://abc.supabase.co/storage/v1/object/public/"
        );
    }

    #[test]
    fn storage_path_accepts_own_uploads_only() {
        let p = storage_public_prefix(SB, "eco-photos");
        assert_eq!(
            storage_object_path(&format!("{p}0f8e-11aa.jpg"), &p),
            Some("0f8e-11aa.jpg")
        );
        assert_eq!(storage_object_path(&format!("{p}user/abc.jpg"), &p), Some("user/abc.jpg"));
        // obcy host z `/eco-photos/` w ścieżce
        assert_eq!(
            storage_object_path("https://evil.example/x/eco-photos/victim.jpg", &p),
            None
        );
        // inny bucket naszego projektu
        assert_eq!(
            storage_object_path(&format!("{SB}/storage/v1/object/public/other/a.jpg"), &p),
            None
        );
        // traversal / kodowanie / query / puste segmenty
        for bad in ["../a.jpg", "a/../b.jpg", "%2e%2e/a.jpg", "a.jpg?x=1", "a.jpg#f", "a//b.jpg", "", "a b.jpg"] {
            assert_eq!(storage_object_path(&format!("{p}{bad}"), &p), None, "{bad}");
        }
    }

    #[test]
    fn optional_storage_url_rules() {
        let p = storage_public_prefix(SB, "eco-photos");
        let mut e = Vec::new();
        check_optional_storage_url(&mut e, "u", None, Some(&p));
        check_optional_storage_url(&mut e, "u", Some("  "), Some(&p));
        check_optional_storage_url(&mut e, "u", Some(&format!("{p}a.jpg")), Some(&p));
        assert!(e.is_empty());
        check_optional_storage_url(&mut e, "u", Some("https://example.com/a.jpg"), Some(&p));
        assert_eq!(e.len(), 1);
        // bez SUPABASE_URL żaden URL nie przechodzi
        check_optional_storage_url(&mut e, "u", Some(&format!("{p}a.jpg")), None);
        assert_eq!(e.len(), 2);
    }

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
