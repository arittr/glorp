use crate::game::calibration::CalibrationBaseline;

pub fn smear_catchup_delta(token_amount: f64, baseline: CalibrationBaseline) -> Vec<f64> {
    let token_amount = token_amount.max(0.0);
    if token_amount == 0.0 {
        return Vec::new();
    }

    let daily = baseline.daily_effective_tokens.max(1.0);
    let bucket_count = ((token_amount / (daily * 0.125)).ceil() as usize).clamp(6, 12);
    let mut buckets = vec![token_amount / bucket_count as f64; bucket_count];
    let max_bucket = daily * 0.25;
    for bucket in &mut buckets {
        *bucket = bucket.min(max_bucket);
    }
    buckets
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // These properties pin the contract of `smear_catchup_delta`:
    //
    // 1. `bucket_count_is_six_to_twelve` — for any positive delta the result has
    //    between 6 and 12 buckets inclusive. This is the documented animation
    //    smoothing window; a refactor that drops below 6 or balloons above 12
    //    would break the renderer's catchup pacing.
    //
    // 2. `each_bucket_at_or_below_daily_cap` — every individual bucket is at
    //    most `daily * 0.25`. This is the per-bucket clamp that prevents a
    //    single catchup tick from exceeding a quarter of one day's calibrated
    //    pace; downstream XP math assumes this bound.
    //
    // 3. `sum_at_or_below_input` — the total of all buckets never exceeds the
    //    input token amount. This documents an intentional silent clipping
    //    behavior: for `token_amount > daily * 3.0` the per-bucket
    //    cap times 12 buckets is `daily * 3.0`, so anything beyond that point
    //    is silently dropped on the floor. Anyone changing this should know
    //    they are changing a load-bearing semantic decision (huge backfills
    //    cannot grant unbounded XP in one poll).
    proptest! {
        #[test]
        fn bucket_count_is_six_to_twelve(
            token_amount in 1.0..1e12f64,
            daily in 1.0..1e10f64,
        ) {
            let baseline = CalibrationBaseline { daily_effective_tokens: daily };
            let buckets = smear_catchup_delta(token_amount, baseline);
            prop_assert!(buckets.len() >= 6 && buckets.len() <= 12,
                "expected 6..=12 buckets, got {}", buckets.len());
        }

        #[test]
        fn each_bucket_at_or_below_daily_cap(
            token_amount in 1.0..1e12f64,
            daily in 1.0..1e10f64,
        ) {
            let baseline = CalibrationBaseline { daily_effective_tokens: daily };
            let buckets = smear_catchup_delta(token_amount, baseline);
            let cap = daily * 0.25;
            for (idx, bucket) in buckets.iter().enumerate() {
                prop_assert!(*bucket <= cap,
                    "bucket {} = {} exceeds daily*0.25 cap {}", idx, bucket, cap);
                prop_assert!(*bucket >= 0.0,
                    "bucket {} = {} is negative", idx, bucket);
            }
        }

        #[test]
        fn sum_at_or_below_input(
            token_amount in 1.0..1e12f64,
            daily in 1.0..1e10f64,
        ) {
            let baseline = CalibrationBaseline { daily_effective_tokens: daily };
            let buckets = smear_catchup_delta(token_amount, baseline);
            let sum: f64 = buckets.iter().sum();
            // Allow a small tolerance for floating-point summation error.
            let tolerance = (token_amount.abs() * 1e-9).max(1e-9);
            prop_assert!(sum <= token_amount + tolerance,
                "sum {} exceeds input {} (tolerance {})", sum, token_amount, tolerance);
        }
    }
}
