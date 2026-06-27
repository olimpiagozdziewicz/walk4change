use serde::Serialize;

use crate::error::{AppError, FieldError};

/// Validated, bounded offset-pagination parameters.
#[derive(Debug)]
pub struct Pagination {
    pub page: i64,
    pub per_page: i64,
}

impl Pagination {
    /// Validate and construct pagination from raw query params.
    ///
    /// Defaults: `page = 1`, `per_page = 20`.
    /// Constraints:
    /// - `page >= 1`
    /// - `1 <= per_page <= 100`  (> 100 → 422 Validation)
    pub fn from_query(page: Option<i64>, per_page: Option<i64>) -> Result<Self, AppError> {
        let page = page.unwrap_or(1);
        let per_page = per_page.unwrap_or(20);

        let mut errors: Vec<FieldError> = Vec::new();

        if page < 1 {
            errors.push(FieldError {
                field: "page".into(),
                message: "must be >= 1".into(),
                code: "OUT_OF_RANGE".into(),
            });
        }
        if per_page < 1 {
            errors.push(FieldError {
                field: "per_page".into(),
                message: "must be >= 1".into(),
                code: "OUT_OF_RANGE".into(),
            });
        } else if per_page > 100 {
            errors.push(FieldError {
                field: "per_page".into(),
                message: "must be <= 100".into(),
                code: "OUT_OF_RANGE".into(),
            });
        }

        if !errors.is_empty() {
            return Err(AppError::Validation(errors));
        }

        Ok(Pagination { page, per_page })
    }

    /// SQL `OFFSET` for this page.
    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.per_page
    }
}

/// Pagination metadata included in paginated API responses.
#[derive(Debug, Serialize)]
pub struct PageMeta {
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

impl PageMeta {
    /// Compute metadata given `total` rows and the applied pagination.
    ///
    /// `total_pages` is at least 1 even when there are no rows, so callers
    /// never receive a zero page count.
    pub fn new(total: i64, pagination: &Pagination) -> Self {
        let total_pages = ((total + pagination.per_page - 1) / pagination.per_page).max(1);
        PageMeta {
            total,
            page: pagination.page,
            per_page: pagination.per_page,
            total_pages,
        }
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_applied() {
        let p = Pagination::from_query(None, None).unwrap();
        assert_eq!(p.page, 1);
        assert_eq!(p.per_page, 20);
        assert_eq!(p.offset(), 0);
    }

    #[test]
    fn page3_offset_is_correct() {
        let p = Pagination::from_query(Some(3), Some(10)).unwrap();
        assert_eq!(p.offset(), 20);
    }

    #[test]
    fn per_page_capped_at_100_rejects() {
        let err = Pagination::from_query(None, Some(101)).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn per_page_exactly_100_is_accepted() {
        let p = Pagination::from_query(None, Some(100)).unwrap();
        assert_eq!(p.per_page, 100);
    }

    #[test]
    fn page_zero_rejects() {
        let err = Pagination::from_query(Some(0), None).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn per_page_zero_rejects() {
        let err = Pagination::from_query(None, Some(0)).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn page_meta_total_pages_rounds_up() {
        let p = Pagination::from_query(Some(1), Some(2)).unwrap();
        let m = PageMeta::new(3, &p);
        assert_eq!(m.total_pages, 2);
    }

    #[test]
    fn page_meta_empty_total_gives_one_page() {
        let p = Pagination::from_query(None, None).unwrap();
        let m = PageMeta::new(0, &p);
        assert_eq!(m.total_pages, 1);
    }
}
