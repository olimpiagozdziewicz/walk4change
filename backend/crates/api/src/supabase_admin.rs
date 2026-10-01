//! Supabase Auth Admin API (service-role) — used ONLY by account deletion
//! (RODO, 2026-10-01): when a SeaSteps account is deleted, the matching user in
//! Supabase Auth (created by `signInWithOtp`) must go too, otherwise the
//! e-mail address and auth metadata outlive the account.
//!
//! Mapping: the backend's `users.id` is NOT the Supabase user id (the
//! `/auth/supabase` exchange creates its own UUID and matches by e-mail), so
//! the Supabase user is found by e-mail through the Admin API (`filter` does
//! a substring ILIKE server-side; we then require an exact, case-insensitive
//! match). We deliberately do NOT touch `auth.users` through the SQL pool:
//! that schema belongs to Supabase's GoTrue (`supabase_auth_admin`), direct
//! DML bypasses its bookkeeping, and the pool role's rights there are not
//! guaranteed.

use serde::Deserialize;

use crate::config::AppConfig;

/// Service-role credentials for the Admin API.
#[derive(Clone)]
pub struct SupabaseAdmin {
    url: String,
    key: String,
}

#[derive(Deserialize)]
struct AdminUser {
    id: String,
    email: Option<String>,
}

#[derive(Deserialize)]
struct AdminUserList {
    #[serde(default)]
    users: Vec<AdminUser>,
}

/// Outcome of [`SupabaseAdmin::delete_user_by_email`].
#[derive(Debug, PartialEq, Eq)]
pub enum AuthPurge {
    /// A matching Supabase Auth user existed and was deleted.
    Deleted,
    /// No Supabase Auth user with this e-mail (e.g. password-only account).
    NotFound,
}

impl SupabaseAdmin {
    /// `None` when `SUPABASE_URL` or the service key is not configured.
    pub fn from_config(cfg: &AppConfig) -> Option<Self> {
        match (&cfg.supabase_url, &cfg.supabase_service_key) {
            (Some(url), Some(key)) => Some(Self {
                url: url.trim_end_matches('/').to_string(),
                key: key.clone(),
            }),
            _ => None,
        }
    }

    fn get(&self, path: &str) -> reqwest::RequestBuilder {
        crate::util::http::client()
            .get(format!("{}{path}", self.url))
            .header("apikey", &self.key)
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", self.key))
    }

    /// Find the Supabase Auth user with exactly this e-mail and delete it.
    ///
    /// Errors are returned as plain strings (never containing the key or the
    /// e-mail) so the caller can log them and roll back.
    pub async fn delete_user_by_email(&self, email: &str) -> Result<AuthPurge, String> {
        let wanted = email.trim().to_lowercase();
        if wanted.is_empty() {
            return Ok(AuthPurge::NotFound);
        }

        // `filter` is a server-side hint, not a documented contract: if GoTrue
        // ignores it we get the unfiltered list. So page through until a short
        // page (or the safety cap) and match exactly on our side either way.
        const PER_PAGE: usize = 100;
        const MAX_PAGES: usize = 100; // 10 000 users — far above today's scale
        let mut ids: Vec<String> = Vec::new();
        for page in 1..=MAX_PAGES {
            let page_s = page.to_string();
            let per_page_s = PER_PAGE.to_string();
            let resp = self
                .get("/auth/v1/admin/users")
                .query(&[
                    ("filter", wanted.as_str()),
                    ("page", page_s.as_str()),
                    ("per_page", per_page_s.as_str()),
                ])
                .send()
                .await
                .map_err(|e| format!("admin list users: {}", e.without_url()))?;
            if !resp.status().is_success() {
                return Err(format!("admin list users: HTTP {}", resp.status()));
            }
            let list: AdminUserList = resp
                .json()
                .await
                .map_err(|e| format!("admin list users: bad body: {}", e.without_url()))?;
            let page_len = list.users.len();
            ids.extend(
                list.users
                    .into_iter()
                    .filter(|u| {
                        u.email
                            .as_deref()
                            .map(|e| e.trim().to_lowercase() == wanted)
                            .unwrap_or(false)
                    })
                    .map(|u| u.id),
            );
            if page_len < PER_PAGE {
                break;
            }
            if page == MAX_PAGES {
                return Err("admin list users: page cap reached".into());
            }
        }

        if ids.is_empty() {
            return Ok(AuthPurge::NotFound);
        }

        for id in &ids {
            // Supabase user ids are UUIDs; refuse anything else before
            // interpolating into the path.
            if uuid::Uuid::parse_str(id).is_err() {
                return Err("admin list users: non-uuid id".into());
            }
            let resp = crate::util::http::client()
                .delete(format!("{}/auth/v1/admin/users/{id}", self.url))
                .header("apikey", &self.key)
                .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", self.key))
                .send()
                .await
                .map_err(|e| format!("admin delete user: {}", e.without_url()))?;
            let status = resp.status();
            // 404 = already gone (e.g. retry after a partial failure) → fine.
            if !status.is_success() && status != reqwest::StatusCode::NOT_FOUND {
                return Err(format!("admin delete user: HTTP {status}"));
            }
        }
        Ok(AuthPurge::Deleted)
    }
}
