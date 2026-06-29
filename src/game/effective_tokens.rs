use crate::config::AppConfig;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TokenBuckets {
    pub uncached_input: u64,
    pub output: u64,
    pub cache_creation: u64,
    pub cache_read: u64,
    pub reasoning_output: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectiveTokenWeights {
    pub cache_read_weight: f64,
}

impl Default for EffectiveTokenWeights {
    fn default() -> Self {
        Self { cache_read_weight: 0.03 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectiveTokenResult {
    pub effective_tokens: f64,
    pub display_cost_usd: Option<f64>,
}

impl EffectiveTokenWeights {
    pub fn from_config(config: AppConfig) -> Self {
        Self {
            cache_read_weight: config.cache_read_weight,
        }
    }

    pub fn compute(&self, buckets: TokenBuckets) -> f64 {
        buckets.uncached_input as f64
            + buckets.output as f64
            + buckets.cache_creation as f64
            + self.cache_read_weight * buckets.cache_read as f64
    }

    pub fn compute_with_display_cost(
        &self,
        buckets: TokenBuckets,
        display_cost_usd: Option<f64>,
    ) -> EffectiveTokenResult {
        EffectiveTokenResult {
            effective_tokens: self.compute(buckets),
            display_cost_usd,
        }
    }
}
