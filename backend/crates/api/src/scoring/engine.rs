use rust_decimal::Decimal;

use crate::scoring::config::ScoringConfig;

/// Input for scoring a single GPS segment.
pub struct SpatialInput {
    pub segment_meters: f64,
    pub dt_secs: f64,
    pub nature_mult: Decimal,
    pub companions: i32,
}

/// Output of scoring a single GPS segment.
pub struct ScoredSegment {
    pub effective_meters: Decimal,
    pub together_mult: Decimal,
    pub points: Decimal,
}

/// Returns the together-multiplier based on companion count.
/// 0 → mult_solo; 1 → mult_pair; >=2 → mult_group.
pub fn together_mult(cfg: &ScoringConfig, companions: i32) -> Decimal {
    match companions {
        0 => cfg.mult_solo,
        1 => cfg.mult_pair,
        _ => cfg.mult_group,
    }
}

/// Scores a single GPS segment.
///
/// Teleport guard: if `dt_secs <= 0` or `segment_meters / dt_secs > max_speed_mps`,
/// effective_meters is set to zero and points = 0.
///
/// Points formula:
/// - `stack == true`:  `(eff_m / meters_per_point) * nature_mult * together_mult`
/// - `stack == false`: `(eff_m / meters_per_point) * max(nature_mult, together_mult)`
///
/// The per-second ceiling (`max_points_per_second`) is applied by the caller (Task 13).
pub fn score_segment(cfg: &ScoringConfig, input: &SpatialInput) -> ScoredSegment {
    let tm = together_mult(cfg, input.companions);

    let effective_meters = if input.dt_secs <= 0.0
        || input.segment_meters / input.dt_secs > cfg.max_speed_mps
    {
        Decimal::ZERO
    } else {
        Decimal::from_f64_retain(input.segment_meters).unwrap_or_default()
    };

    let points = if effective_meters == Decimal::ZERO {
        Decimal::ZERO
    } else {
        let base = effective_meters / cfg.meters_per_point;
        if cfg.stack {
            base * input.nature_mult * tm
        } else {
            let multiplier = input.nature_mult.max(tm);
            base * multiplier
        }
    };

    ScoredSegment {
        effective_meters,
        together_mult: tm,
        points,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn default_cfg() -> ScoringConfig {
        ScoringConfig::default()
    }

    fn cfg_no_stack() -> ScoringConfig {
        ScoringConfig {
            stack: false,
            ..ScoringConfig::default()
        }
    }

    /// Normalize trailing zeros so `1.0000` == `1` in assertions.
    fn n(d: Decimal) -> Decimal {
        d.normalize()
    }

    // ─── companion mapping ────────────────────────────────────────────────────

    #[test]
    fn companion_map_0_is_solo() {
        let cfg = default_cfg();
        assert_eq!(together_mult(&cfg, 0), cfg.mult_solo);
    }

    #[test]
    fn companion_map_1_is_pair() {
        let cfg = default_cfg();
        assert_eq!(together_mult(&cfg, 1), cfg.mult_pair);
    }

    #[test]
    fn companion_map_2_is_group() {
        let cfg = default_cfg();
        assert_eq!(together_mult(&cfg, 2), cfg.mult_group);
    }

    #[test]
    fn companion_map_5_is_group() {
        let cfg = default_cfg();
        assert_eq!(together_mult(&cfg, 5), cfg.mult_group);
    }

    // ─── score_segment: valid segments ───────────────────────────────────────

    #[test]
    fn solo_flat_ground_baseline() {
        // 100 m, 100 s → speed 1 m/s < 8 cap; nature 1.0, solo → points = 1.0
        let cfg = default_cfg();
        let input = SpatialInput {
            segment_meters: 100.0,
            dt_secs: 100.0,
            nature_mult: Decimal::new(1, 0),
            companions: 0,
        };
        let seg = score_segment(&cfg, &input);
        assert_eq!(n(seg.points), Decimal::new(1, 0));
    }

    #[test]
    fn nature_3x_triples_points() {
        // 100 m, 100 s, nature 3.0, solo → points = 3.0
        let cfg = default_cfg();
        let input = SpatialInput {
            segment_meters: 100.0,
            dt_secs: 100.0,
            nature_mult: Decimal::new(3, 0),
            companions: 0,
        };
        let seg = score_segment(&cfg, &input);
        assert_eq!(n(seg.points), Decimal::new(3, 0));
    }

    #[test]
    fn pair_multiplier_1_5() {
        // 100 m, 100 s, nature 1.0, 1 companion → points = 1.5
        let cfg = default_cfg();
        let input = SpatialInput {
            segment_meters: 100.0,
            dt_secs: 100.0,
            nature_mult: Decimal::new(1, 0),
            companions: 1,
        };
        let seg = score_segment(&cfg, &input);
        assert_eq!(n(seg.points), Decimal::new(15, 1));
    }

    #[test]
    fn group_multiplier_2_0() {
        // 100 m, 100 s, nature 1.0, 2 companions → points = 2.0
        let cfg = default_cfg();
        let input = SpatialInput {
            segment_meters: 100.0,
            dt_secs: 100.0,
            nature_mult: Decimal::new(1, 0),
            companions: 2,
        };
        let seg = score_segment(&cfg, &input);
        assert_eq!(n(seg.points), Decimal::new(2, 0));
    }

    #[test]
    fn stack_true_multiplies_both() {
        // 100 m, 100 s, nature 3.0, pair (1 companion) → 1 * 3 * 1.5 = 4.5
        let cfg = default_cfg();
        assert!(cfg.stack, "default config must have stack=true");
        let input = SpatialInput {
            segment_meters: 100.0,
            dt_secs: 100.0,
            nature_mult: Decimal::new(3, 0),
            companions: 1,
        };
        let seg = score_segment(&cfg, &input);
        assert_eq!(n(seg.points), Decimal::from_str("4.5").unwrap());
    }

    #[test]
    fn stack_false_uses_max_of_nature_and_together() {
        // stack=false, nature 3.0, pair 1.5 → uses max(3.0, 1.5) = 3.0
        let cfg = cfg_no_stack();
        let input = SpatialInput {
            segment_meters: 100.0,
            dt_secs: 100.0,
            nature_mult: Decimal::new(3, 0),
            companions: 1,
        };
        let seg = score_segment(&cfg, &input);
        assert_eq!(n(seg.points), Decimal::new(3, 0));
    }

    // ─── score_segment: invalid / teleport ───────────────────────────────────

    #[test]
    fn teleport_speed_over_cap_returns_zero() {
        // 100000 m in 1 s → 100 000 m/s >> 8 m/s cap → invalid
        let cfg = default_cfg();
        let input = SpatialInput {
            segment_meters: 100_000.0,
            dt_secs: 1.0,
            nature_mult: Decimal::new(3, 0),
            companions: 0,
        };
        let seg = score_segment(&cfg, &input);
        assert_eq!(seg.effective_meters, Decimal::ZERO);
        assert_eq!(seg.points, Decimal::ZERO);
    }

    #[test]
    fn dt_zero_returns_zero() {
        let cfg = default_cfg();
        let input = SpatialInput {
            segment_meters: 100.0,
            dt_secs: 0.0,
            nature_mult: Decimal::new(1, 0),
            companions: 0,
        };
        let seg = score_segment(&cfg, &input);
        assert_eq!(seg.effective_meters, Decimal::ZERO);
        assert_eq!(seg.points, Decimal::ZERO);
    }

    #[test]
    fn dt_negative_returns_zero() {
        let cfg = default_cfg();
        let input = SpatialInput {
            segment_meters: 100.0,
            dt_secs: -5.0,
            nature_mult: Decimal::new(1, 0),
            companions: 0,
        };
        let seg = score_segment(&cfg, &input);
        assert_eq!(seg.effective_meters, Decimal::ZERO);
        assert_eq!(seg.points, Decimal::ZERO);
    }
}
