use rust_decimal::Decimal;

/// Scoring engine configuration.
/// Decimal fields: meters_per_point, multipliers, max_points_per_second.
/// Primitive fields: booleans, speeds, and time windows.
#[derive(Debug, Clone)]
pub struct ScoringConfig {
    /// Distance (in meters) required to earn one point.
    pub meters_per_point: Decimal,
    /// Multiplier when walking solo (1.0).
    pub mult_solo: Decimal,
    /// Multiplier when walking with exactly one friend (1.5).
    pub mult_pair: Decimal,
    /// Multiplier when walking in a group of 3+ (2.0).
    pub mult_group: Decimal,
    /// Default nature zone multiplier (3.0).
    pub nature_default: Decimal,
    /// Whether multipliers stack multiplicatively (true) or are applied separately.
    pub stack: bool,
    /// Maximum plausible walking/running speed in m/s; segments above this are dropped.
    pub max_speed_mps: f64,
    /// GPS-jitter deadband (meters): segments shorter than this are treated as
    /// noise (stationary drift) and credited zero distance.
    pub min_segment_meters: f64,
    /// Maximum acceptable GPS accuracy radius (meters). Pings reported with a
    /// worse (larger) accuracy are dropped before scoring — a poor fix drifts
    /// several metres while standing still and would otherwise mint points.
    pub max_accuracy_meters: f64,
    /// Window (seconds) for counting companions active in the same session.
    pub ping_window_secs: i64,
    /// Maximum allowed clock skew (seconds) between client `recorded_at` and server time.
    pub recorded_at_tolerance_secs: i64,
    /// Per-second ceiling on points a single user can earn (anti-fraud layer).
    pub max_points_per_second: Decimal,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            meters_per_point: Decimal::new(100, 0),     // 100 m / point
            mult_solo: Decimal::new(1, 0),              // 1.0
            mult_pair: Decimal::new(15, 1),             // 1.5
            mult_group: Decimal::new(2, 0),             // 2.0
            nature_default: Decimal::new(3, 0),         // 3.0
            stack: true,
            max_speed_mps: 8.0,                          // ~28.8 km/h (fast run)
            min_segment_meters: 5.0,                     // GPS jitter deadband
            max_accuracy_meters: 35.0,                   // drop poor-fix pings
            ping_window_secs: 60,
            recorded_at_tolerance_secs: 45,
            max_points_per_second: Decimal::new(5, 0),  // 5.0 pt/s ceiling
        }
    }
}

impl ScoringConfig {
    /// Load from environment variables, falling back to defaults for any missing value.
    pub fn from_env_or_default() -> Self {
        let mut cfg = Self::default();

        if let Ok(v) = std::env::var("SCORING_MAX_SPEED_MPS") {
            if let Ok(n) = v.parse::<f64>() {
                cfg.max_speed_mps = n;
            }
        }
        if let Ok(v) = std::env::var("SCORING_MIN_SEGMENT_METERS") {
            if let Ok(n) = v.parse::<f64>() {
                cfg.min_segment_meters = n;
            }
        }
        if let Ok(v) = std::env::var("SCORING_MAX_ACCURACY_METERS") {
            if let Ok(n) = v.parse::<f64>() {
                cfg.max_accuracy_meters = n;
            }
        }
        if let Ok(v) = std::env::var("SCORING_PING_WINDOW_SECS") {
            if let Ok(n) = v.parse::<i64>() {
                cfg.ping_window_secs = n;
            }
        }
        if let Ok(v) = std::env::var("SCORING_RECORDED_AT_TOLERANCE_SECS") {
            if let Ok(n) = v.parse::<i64>() {
                cfg.recorded_at_tolerance_secs = n;
            }
        }
        if let Ok(v) = std::env::var("SCORING_STACK") {
            cfg.stack = v.to_lowercase() != "false";
        }

        cfg
    }
}
