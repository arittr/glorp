/// Ten-minute token count at which the companion pace gauge reaches roughly
/// 63.2 percent. Shared by every presentation backend.
pub const PACE_SOFT_CAP_10M_TOKENS: f64 = 15_000_000.0;

pub fn companion_pace_fraction(current_10m_tokens: f64) -> f64 {
    if !current_10m_tokens.is_finite() || current_10m_tokens <= 0.0 {
        return 0.0;
    }
    (1.0 - (-current_10m_tokens / PACE_SOFT_CAP_10M_TOKENS).exp()).clamp(0.0, 1.0)
}

pub fn daily_fraction_for_gauge(fraction_of_yesterday: Option<f64>) -> f64 {
    fraction_of_yesterday
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.clamp(0.0, 1.0))
        .unwrap_or(0.0)
}

pub fn daily_overage_marker_fraction(fraction_of_yesterday: Option<f64>) -> f64 {
    fraction_of_yesterday
        .filter(|value| value.is_finite() && *value > 1.0)
        .map(|value| value - 1.0)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pace_fraction_is_bounded_monotonic_and_rejects_invalid_values() {
        assert_eq!(PACE_SOFT_CAP_10M_TOKENS, 15_000_000.0);
        for invalid in [f64::NEG_INFINITY, -1.0, 0.0, f64::INFINITY, f64::NAN] {
            assert_eq!(companion_pace_fraction(invalid), 0.0);
        }
        let at_cap = companion_pace_fraction(PACE_SOFT_CAP_10M_TOKENS);
        let at_twice_cap = companion_pace_fraction(PACE_SOFT_CAP_10M_TOKENS * 2.0);
        assert!((at_cap - 0.632).abs() < 0.002);
        assert!((at_twice_cap - 0.865).abs() < 0.002);
        assert!(at_cap < at_twice_cap);
        assert!(companion_pace_fraction(PACE_SOFT_CAP_10M_TOKENS * 100.0) <= 1.0);
    }

    #[test]
    fn daily_fraction_clamps_to_the_base_lane() {
        for invalid in [
            None,
            Some(f64::NEG_INFINITY),
            Some(-1.0),
            Some(0.0),
            Some(f64::INFINITY),
            Some(f64::NAN),
        ] {
            assert_eq!(daily_fraction_for_gauge(invalid), 0.0);
        }
        assert_eq!(daily_fraction_for_gauge(Some(0.25)), 0.25);
        assert_eq!(daily_fraction_for_gauge(Some(1.0)), 1.0);
        assert_eq!(daily_fraction_for_gauge(Some(2.5)), 1.0);
    }

    #[test]
    fn daily_overage_projects_only_the_excess_lane() {
        for inactive in [
            None,
            Some(f64::NEG_INFINITY),
            Some(-1.0),
            Some(0.0),
            Some(1.0),
            Some(f64::INFINITY),
            Some(f64::NAN),
        ] {
            assert_eq!(daily_overage_marker_fraction(inactive), 0.0);
        }
        assert!((daily_overage_marker_fraction(Some(1.07)) - 0.07).abs() < 0.001);
        assert_eq!(daily_overage_marker_fraction(Some(2.0)), 1.0);
        assert!((daily_overage_marker_fraction(Some(2.62)) - 1.62).abs() < 0.001);
        assert_eq!(daily_overage_marker_fraction(Some(3.0)), 2.0);
    }
}
