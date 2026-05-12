/// Format an effective-tokens count for compact display.
///
/// Clamps negative values to zero, then renders raw integers below 1k, a
/// one-decimal `k` suffix below 1M, and a one-decimal `M` suffix above.
pub fn format_tokens(value: f64) -> String {
    let value = value.max(0.0);
    if value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("{:.1}k", value / 1_000.0)
    } else {
        format!("{value:.0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_tokens_renders_canonical_units() {
        assert_eq!(format_tokens(0.0), "0");
        assert_eq!(format_tokens(750.0), "750");
        assert_eq!(format_tokens(999.0), "999");
        assert_eq!(format_tokens(1_000.0), "1.0k");
        // 1250 / 1000 = 1.25; `{:.1}` rounds half-to-even to 1.2.
        assert_eq!(format_tokens(1_250.0), "1.2k");
        assert_eq!(format_tokens(123_456.0), "123.5k");
        // 999_999 / 1000 rounds half-to-even to 1000.0 under `{:.1}`.
        assert_eq!(format_tokens(999_999.0), "1000.0k");
        assert_eq!(format_tokens(1_000_000.0), "1.0M");
        assert_eq!(format_tokens(2_345_678.0), "2.3M");
        // Tiny positive floats round down to "0".
        assert_eq!(format_tokens(0.4), "0");
        // Negatives clamp to zero.
        assert_eq!(format_tokens(-500.0), "0");
    }
}
