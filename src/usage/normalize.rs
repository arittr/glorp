use crate::game::effective_tokens::{EffectiveTokenWeights, TokenBuckets};
use crate::usage::provider::ProviderDiagnostic;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawTokenTotals {
    pub uncached_input: u64,
    pub output: u64,
    pub cache_creation: u64,
    pub cache_read: u64,
    pub reasoning_output: u64,
}

impl RawTokenTotals {
    pub fn effective_tokens(self, weights: EffectiveTokenWeights) -> f64 {
        weights.compute(TokenBuckets {
            uncached_input: self.uncached_input,
            output: self.output,
            cache_creation: self.cache_creation,
            cache_read: self.cache_read,
            reasoning_output: self.reasoning_output,
        })
    }

    pub fn positive_delta_since(self, previous: Self) -> Option<Self> {
        if self.uncached_input < previous.uncached_input
            || self.output < previous.output
            || self.cache_creation < previous.cache_creation
            || self.cache_read < previous.cache_read
            || self.reasoning_output < previous.reasoning_output
        {
            return None;
        }

        Some(Self {
            uncached_input: self.uncached_input - previous.uncached_input,
            output: self.output - previous.output,
            cache_creation: self.cache_creation - previous.cache_creation,
            cache_read: self.cache_read - previous.cache_read,
            reasoning_output: self.reasoning_output - previous.reasoning_output,
        })
    }

    pub fn has_positive_effective_bucket(self) -> bool {
        self.uncached_input > 0 || self.output > 0 || self.cache_creation > 0 || self.cache_read > 0
    }
}

#[derive(Debug, Clone)]
pub struct NormalizedUsageRecord {
    pub provider_surface: String,
    pub period_start: String,
    pub model: Option<String>,
    pub raw_totals: RawTokenTotals,
    pub display_cost_usd: Option<f64>,
    pub confidence: String,
}

pub fn normalize_usage_json(
    provider_surface: &str,
    text: &str,
) -> std::result::Result<Vec<NormalizedUsageRecord>, ProviderDiagnostic> {
    let value: Value = serde_json::from_str(text).map_err(|_| ProviderDiagnostic {
        provider_surface: provider_surface.to_string(),
        code: "invalid_json".to_string(),
        message: format!("{provider_surface} returned invalid_json"),
    })?;

    normalize_usage_value(provider_surface, &value)
}

fn normalize_usage_value(
    provider_surface: &str,
    value: &Value,
) -> std::result::Result<Vec<NormalizedUsageRecord>, ProviderDiagnostic> {
    let rows = value
        .get("daily")
        .or_else(|| value.get("data"))
        .and_then(Value::as_array)
        .ok_or_else(|| ProviderDiagnostic {
            provider_surface: provider_surface.to_string(),
            code: "missing_token_fields".to_string(),
            message: format!("{provider_surface} missing fields daily/data"),
        })?;

    let mut normalized = Vec::new();
    for row in rows {
        if provider_surface == "codex" {
            normalized.extend(normalize_codex_row(provider_surface, row)?);
        } else {
            normalized.extend(normalize_claude_row(provider_surface, row)?);
        }
    }
    normalized.sort_by(|a, b| {
        a.period_start
            .cmp(&b.period_start)
            .then_with(|| a.model.cmp(&b.model))
    });
    Ok(normalized)
}

fn normalize_claude_row(
    provider_surface: &str,
    row: &Value,
) -> std::result::Result<Vec<NormalizedUsageRecord>, ProviderDiagnostic> {
    let period_start = required_string(provider_surface, row, "date")?;
    if let Some(breakdowns) = row.get("modelBreakdowns").and_then(Value::as_array) {
        let mut records = Vec::new();
        for model_row in breakdowns {
            let raw_totals = claude_totals(provider_surface, model_row)?;
            records.push(NormalizedUsageRecord {
                provider_surface: provider_surface.to_string(),
                period_start: period_start.clone(),
                model: optional_string(model_row, "modelName"),
                raw_totals,
                display_cost_usd: optional_f64(model_row, "cost"),
                confidence: "local-log-derived".to_string(),
            });
        }
        if !records.is_empty() {
            return Ok(records);
        }
    }

    let raw_totals = claude_totals(provider_surface, row)?;
    Ok(vec![NormalizedUsageRecord {
        provider_surface: provider_surface.to_string(),
        period_start,
        model: first_string(row, "modelsUsed"),
        raw_totals,
        display_cost_usd: optional_f64(row, "totalCost"),
        confidence: "local-log-derived".to_string(),
    }])
}

fn normalize_codex_row(
    provider_surface: &str,
    row: &Value,
) -> std::result::Result<Vec<NormalizedUsageRecord>, ProviderDiagnostic> {
    let period_start = required_string(provider_surface, row, "date")?;
    if let Some(models) = row.get("models").and_then(Value::as_object) {
        let mut records = Vec::new();
        for (model, model_row) in models {
            let raw_totals = codex_totals(provider_surface, model_row)?;
            records.push(NormalizedUsageRecord {
                provider_surface: provider_surface.to_string(),
                period_start: period_start.clone(),
                model: Some(model.clone()),
                raw_totals,
                display_cost_usd: optional_f64(model_row, "costUSD")
                    .or_else(|| optional_f64(row, "costUSD")),
                confidence: "local-log-derived".to_string(),
            });
        }
        if !records.is_empty() {
            return Ok(records);
        }
    }

    let raw_totals = codex_totals(provider_surface, row)?;
    Ok(vec![NormalizedUsageRecord {
        provider_surface: provider_surface.to_string(),
        period_start,
        model: None,
        raw_totals,
        display_cost_usd: optional_f64(row, "costUSD"),
        confidence: "local-log-derived".to_string(),
    }])
}

fn claude_totals(
    provider_surface: &str,
    value: &Value,
) -> std::result::Result<RawTokenTotals, ProviderDiagnostic> {
    Ok(RawTokenTotals {
        uncached_input: required_u64(provider_surface, value, "inputTokens")?,
        output: required_u64(provider_surface, value, "outputTokens")?,
        cache_creation: required_u64(provider_surface, value, "cacheCreationTokens")?,
        cache_read: required_u64(provider_surface, value, "cacheReadTokens")?,
        reasoning_output: optional_u64(value, "reasoningOutputTokens").unwrap_or(0),
    })
}

fn codex_totals(
    provider_surface: &str,
    value: &Value,
) -> std::result::Result<RawTokenTotals, ProviderDiagnostic> {
    // OpenAI/codex semantics: `inputTokens` is the TOTAL prompt size and
    // `cachedInputTokens` is the cached subset of that total. So real
    // uncached input is the difference. Claude's `inputTokens` is already
    // exclusive of cache reads (separate field in Anthropic's API), so
    // that path stays as-is.
    let total_input = required_u64(provider_surface, value, "inputTokens")?;
    let cached_input = required_u64(provider_surface, value, "cachedInputTokens")?;
    Ok(RawTokenTotals {
        uncached_input: total_input.saturating_sub(cached_input),
        output: required_u64(provider_surface, value, "outputTokens")?,
        cache_creation: optional_u64(value, "cacheCreationTokens").unwrap_or(0),
        cache_read: cached_input,
        reasoning_output: optional_u64(value, "reasoningOutputTokens").unwrap_or(0),
    })
}

fn required_string(
    provider_surface: &str,
    value: &Value,
    field: &str,
) -> std::result::Result<String, ProviderDiagnostic> {
    optional_string(value, field).ok_or_else(|| missing(provider_surface, field))
}

fn required_u64(
    provider_surface: &str,
    value: &Value,
    field: &str,
) -> std::result::Result<u64, ProviderDiagnostic> {
    optional_u64(value, field).ok_or_else(|| missing(provider_surface, field))
}

fn missing(provider_surface: &str, field: &str) -> ProviderDiagnostic {
    ProviderDiagnostic {
        provider_surface: provider_surface.to_string(),
        code: "missing_token_fields".to_string(),
        message: format!("{provider_surface} missing field {field}"),
    }
}

fn optional_string(value: &Value, field: &str) -> Option<String> {
    value.get(field)?.as_str().map(ToString::to_string)
}

fn first_string(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)?
        .as_array()?
        .first()?
        .as_str()
        .map(ToString::to_string)
}

fn optional_u64(value: &Value, field: &str) -> Option<u64> {
    value.get(field)?.as_u64()
}

fn optional_f64(value: &Value, field: &str) -> Option<f64> {
    value.get(field)?.as_f64()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::effective_tokens::EffectiveTokenWeights;

    #[test]
    fn codex_uncached_input_subtracts_cached_subset() {
        // Real ccusage-codex emits `inputTokens` as the TOTAL prompt size
        // and `cachedInputTokens` as the cached subset. Make sure we don't
        // double-bill the cached portion at full input weight.
        let row = serde_json::json!({
            "inputTokens": 19_994_265,
            "outputTokens": 118_665,
            "cachedInputTokens": 19_074_688,
            "reasoningOutputTokens": 49_449
        });
        let totals = codex_totals("codex", &row).unwrap();
        assert_eq!(totals.uncached_input, 919_577);
        assert_eq!(totals.cache_read, 19_074_688);

        let weights = EffectiveTokenWeights::default();
        let effective = totals.effective_tokens(weights);
        // Expected: 919_577 + 118_665 + 0 + 19_074_688 * 0.03 ≈ 1_610_482.64
        assert!(
            (1_610_000.0..1_611_000.0).contains(&effective),
            "got {effective}"
        );
    }

    #[test]
    fn claude_input_already_excludes_cache_reads() {
        // Claude's `inputTokens` is genuinely uncached input by API design,
        // so the totals struct should mirror it 1:1.
        let row = serde_json::json!({
            "inputTokens": 100,
            "outputTokens": 200,
            "cacheCreationTokens": 50,
            "cacheReadTokens": 1000
        });
        let totals = claude_totals("claude-code", &row).unwrap();
        assert_eq!(totals.uncached_input, 100);
        assert_eq!(totals.cache_creation, 50);
        assert_eq!(totals.cache_read, 1000);
    }
}
