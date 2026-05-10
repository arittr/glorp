use crate::game::calibration::CalibrationBaseline;

pub fn smear_catchup_delta(effective_tokens: f64, baseline: CalibrationBaseline) -> Vec<f64> {
    let effective_tokens = effective_tokens.max(0.0);
    if effective_tokens == 0.0 {
        return Vec::new();
    }

    let daily = baseline.daily_effective_tokens.max(1.0);
    let bucket_count = ((effective_tokens / (daily * 0.125)).ceil() as usize).clamp(6, 12);
    let mut buckets = vec![effective_tokens / bucket_count as f64; bucket_count];
    let max_bucket = daily * 0.25;
    for bucket in &mut buckets {
        *bucket = bucket.min(max_bucket);
    }
    buckets
}
