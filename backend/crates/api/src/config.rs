use crate::scoring::config::ScoringConfig;

/// Application configuration, loaded from environment variables at startup.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub jwt_secret: String,
    pub bind_addr: String,
    pub cors_origins: Vec<String>,
    pub argon2_m_cost: u32,
    pub argon2_t_cost: u32,
    pub argon2_p_cost: u32,
    pub jwt_ttl_secs: i64,
    pub scoring: ScoringConfig,
    /// Max requests per window for `/api/v1/auth/*` (strict bucket).
    pub rate_limit_auth_max: u32,
    /// Max requests per window for all other routes (moderate bucket).
    pub rate_limit_global_max: u32,
    /// Sliding-window duration in seconds for rate limiting.
    pub rate_limit_window_secs: u64,
    /// Public base URL of the web app, used to build magic-link URLs.
    pub app_url: String,
    /// SMTP settings for magic-link email. `None` => SMTP magic-link disabled.
    pub mail: Option<MailConfig>,
    /// Supabase project URL (e.g. https://<ref>.supabase.co) for token exchange.
    pub supabase_url: Option<String>,
    /// Supabase anon (public) key, used as the `apikey` when validating tokens.
    pub supabase_anon_key: Option<String>,
    /// Supabase service-role key — ONLY for storage cleanup at account
    /// deletion (RODO tail, spec 2026-07-13). Never sent to clients.
    pub supabase_service_key: Option<String>,
}

/// SMTP configuration for sending magic-link emails.
#[derive(Debug, Clone)]
pub struct MailConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub pass: String,
    pub from: String,
}

/// Errors that can occur during configuration loading.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("JWT_SECRET must be at least 32 characters (got fewer)")]
    JwtSecretTooShort,
    #[error("ARGON2_M_COST must be >= 19456 KiB (OWASP minimum)")]
    Argon2MCostTooLow,
    #[error("Missing required environment variable: {0}")]
    MissingEnv(String),
}

impl AppConfig {
    /// Load and validate configuration from environment variables.
    /// Loads `.env` if present (via dotenvy). Fails fast on constraint violations.
    pub fn from_env() -> Result<Self, ConfigError> {
        // Attempt to load .env; silently ignore if absent.
        let _ = dotenvy::dotenv();

        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| ConfigError::MissingEnv("DATABASE_URL".into()))?;

        let jwt_secret = std::env::var("JWT_SECRET")
            .map_err(|_| ConfigError::MissingEnv("JWT_SECRET".into()))?;

        if jwt_secret.len() < 32 {
            return Err(ConfigError::JwtSecretTooShort);
        }

        let bind_addr = std::env::var("BIND_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8080".into());

        let cors_origins: Vec<String> = std::env::var("CORS_ORIGINS")
            .or_else(|_| std::env::var("CORS_ALLOWED_ORIGINS"))
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .collect();

        let argon2_m_cost: u32 = std::env::var("ARGON2_M_COST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(19456);

        if argon2_m_cost < 19456 {
            return Err(ConfigError::Argon2MCostTooLow);
        }

        let argon2_t_cost: u32 = std::env::var("ARGON2_T_COST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);

        let argon2_p_cost: u32 = std::env::var("ARGON2_P_COST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);

        let jwt_ttl_secs: i64 = std::env::var("JWT_TTL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600);

        let rate_limit_auth_max: u32 = std::env::var("RATE_LIMIT_AUTH_MAX")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        let rate_limit_global_max: u32 = std::env::var("RATE_LIMIT_GLOBAL_MAX")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(120);

        let rate_limit_window_secs: u64 = std::env::var("RATE_LIMIT_WINDOW_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        // Public app URL for magic-link emails (falls back to first CORS origin, then localhost).
        let app_url = std::env::var("APP_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| cors_origins.first().cloned().unwrap_or_else(|| "http://localhost:5173".into()));

        // SMTP is optional; magic-link is enabled only when host+user+pass are set.
        let mail = match (
            std::env::var("SMTP_HOST").ok().filter(|s| !s.is_empty()),
            std::env::var("SMTP_USER").ok().filter(|s| !s.is_empty()),
            std::env::var("SMTP_PASS").ok().filter(|s| !s.is_empty()),
        ) {
            (Some(host), Some(user), Some(pass)) => Some(MailConfig {
                port: std::env::var("SMTP_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(587),
                from: std::env::var("SMTP_FROM").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| user.clone()),
                host,
                user,
                pass,
            }),
            _ => None,
        };

        let supabase_url = std::env::var("SUPABASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim_end_matches('/').to_string());
        let supabase_anon_key = std::env::var("SUPABASE_ANON_KEY").ok().filter(|s| !s.trim().is_empty());
        let supabase_service_key = std::env::var("SUPABASE_SERVICE_KEY").ok().filter(|s| !s.trim().is_empty());

        Ok(Self {
            database_url,
            jwt_secret,
            bind_addr,
            cors_origins,
            argon2_m_cost,
            argon2_t_cost,
            argon2_p_cost,
            jwt_ttl_secs,
            scoring: ScoringConfig::from_env_or_default(),
            rate_limit_auth_max,
            rate_limit_global_max,
            rate_limit_window_secs,
            app_url,
            mail,
            supabase_url,
            supabase_anon_key,
            supabase_service_key,
        })
    }

    /// Returns a valid config for unit tests (no DB or .env required).
    /// Uses OWASP-minimum argon2 params and a 40-char JWT secret.
    /// Rate limits are intentionally very high so no existing tests trip them.
    pub fn test_default() -> Self {
        Self {
            database_url: "postgres://localhost/walk4change_test".into(),
            jwt_secret: "test-secret-that-is-at-least-32-chars!!".into(),
            bind_addr: "0.0.0.0:8080".into(),
            cors_origins: vec![],
            argon2_m_cost: 19456,
            argon2_t_cost: 2,
            argon2_p_cost: 1,
            jwt_ttl_secs: 3600,
            scoring: ScoringConfig::default(),
            rate_limit_auth_max: 1_000,
            rate_limit_global_max: 10_000,
            rate_limit_window_secs: 60,
            app_url: "http://localhost:5173".into(),
            mail: None,
            supabase_url: None,
            supabase_anon_key: None,
            supabase_service_key: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn rejects_short_jwt_secret() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("JWT_SECRET", "short");
        std::env::set_var("DATABASE_URL", "postgres://x");
        let err = AppConfig::from_env().unwrap_err();
        std::env::remove_var("JWT_SECRET");
        std::env::remove_var("DATABASE_URL");
        assert!(matches!(err, ConfigError::JwtSecretTooShort));
    }

    #[test]
    fn test_default_is_valid() {
        let cfg = AppConfig::test_default();
        assert!(cfg.jwt_secret.len() >= 32);
        assert!(cfg.argon2_m_cost >= 19456);
        assert_eq!(cfg.argon2_t_cost, 2);
        assert_eq!(cfg.argon2_p_cost, 1);
    }
}
