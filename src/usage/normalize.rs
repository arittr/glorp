use crate::game::effective_tokens::{EffectiveTokenWeights, TokenBuckets};
use crate::usage::identity::SourceIdentity;
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
    pub fn total_tokens(self) -> f64 {
        self.uncached_input as f64
            + self.output as f64
            + self.cache_creation as f64
            + self.cache_read as f64
    }

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

#[derive(Debug, Clone, Default)]
pub struct NormalizedUsageBatch {
    pub records: Vec<NormalizedUsageRecord>,
    pub diagnostics: Vec<ProviderDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct NormalizedUsageRecord {
    pub source_identity: SourceIdentity,
    pub period_start: String,
    pub model: Option<String>,
    pub raw_totals: RawTokenTotals,
    pub display_cost_usd: Option<f64>,
    pub confidence: String,
}

pub fn normalize_usage_json(
    provider_surface: &str,
    text: &str,
) -> std::result::Result<NormalizedUsageBatch, ProviderDiagnostic> {
    let value: Value = serde_json::from_str(text).map_err(|_| ProviderDiagnostic {
        provider_surface: provider_surface.to_string(),
        code: "invalid_json".to_string(),
        message: format!("{provider_surface} returned invalid_json"),
    })?;
    Ok(normalize_usage_value(provider_surface, &value))
}

pub fn normalize_agentsview_json(
    agent: &str,
    text: &str,
) -> std::result::Result<NormalizedUsageBatch, ProviderDiagnostic> {
    let value: Value = serde_json::from_str(text).map_err(|_| ProviderDiagnostic {
        provider_surface: agent.to_string(),
        code: "invalid_json".to_string(),
        message: format!("agentsview {agent} returned invalid_json"),
    })?;
    Ok(normalize_agentsview_value(agent, &value))
}

fn normalize_agentsview_value(agent: &str, value: &Value) -> NormalizedUsageBatch {
    let mut batch = NormalizedUsageBatch::default();
    let Some(rows) = value.get("daily").and_then(Value::as_array) else {
        batch.diagnostics.push(ProviderDiagnostic {
            provider_surface: agent.to_string(),
            code: "missing_daily".to_string(),
            message: format!("agentsview {agent} missing daily"),
        });
        return batch;
    };

    let source = SourceIdentity::from_tokenmaxxing_source(agent);
    for row in rows {
        let period_start = match period_start_field(&source.provider_surface, row) {
            Ok(period) => period,
            Err(diagnostic) => {
                batch.diagnostics.push(diagnostic);
                continue;
            }
        };

        if let Some(breakdowns) = row.get("modelBreakdowns").and_then(Value::as_array) {
            for model_row in breakdowns {
                batch.records.push(NormalizedUsageRecord {
                    source_identity: source.clone(),
                    period_start: period_start.clone(),
                    model: optional_string(model_row, "modelName"),
                    raw_totals: RawTokenTotals {
                        uncached_input: optional_u64(model_row, "inputTokens").unwrap_or(0),
                        output: optional_u64(model_row, "outputTokens").unwrap_or(0),
                        cache_creation: optional_u64(model_row, "cacheCreationTokens").unwrap_or(0),
                        cache_read: optional_u64(model_row, "cacheReadTokens").unwrap_or(0),
                        reasoning_output: optional_u64(model_row, "reasoningOutputTokens")
                            .unwrap_or(0),
                    },
                    display_cost_usd: optional_f64(model_row, "cost"),
                    confidence: "local-log-derived".to_string(),
                });
            }
        }
    }

    batch
}

fn normalize_usage_value(provider_surface: &str, value: &Value) -> NormalizedUsageBatch {
    let mut batch = NormalizedUsageBatch::default();
    let rows = match value
        .get("daily")
        .or_else(|| value.get("data"))
        .and_then(Value::as_array)
    {
        Some(rows) => rows,
        None => {
            batch.diagnostics.push(ProviderDiagnostic {
                provider_surface: provider_surface.to_string(),
                code: "missing_token_fields".to_string(),
                message: format!("{provider_surface} missing fields daily/data"),
            });
            return batch;
        }
    };

    for row in rows {
        match normalize_row(provider_surface, row, &mut batch.diagnostics) {
            Ok(records) => batch.records.extend(records),
            Err(diagnostic) => batch.diagnostics.push(diagnostic),
        }
    }
    batch.records.sort_by(|a, b| {
        a.period_start
            .cmp(&b.period_start)
            .then_with(|| a.model.cmp(&b.model))
            .then_with(|| {
                a.source_identity
                    .provider_surface
                    .cmp(&b.source_identity.provider_surface)
            })
    });
    batch
}

fn source_identity_from_row(provider_surface: &str, row: &Value) -> SourceIdentity {
    if let Some(raw) = optional_string(row, "agent")
        .or_else(|| optional_string(row, "source"))
        .or_else(|| first_string(row, "sources"))
    {
        SourceIdentity::from_raw_agent(&raw)
    } else {
        SourceIdentity::from_provider_surface(provider_surface)
    }
}

fn is_aggregate_all_row(row: &Value) -> bool {
    optional_string(row, "agent")
        .or_else(|| optional_string(row, "source"))
        .or_else(|| first_string(row, "sources"))
        .is_some_and(|s| s.eq_ignore_ascii_case("all"))
}

fn normalize_row(
    provider_surface: &str,
    row: &Value,
    diagnostics: &mut Vec<ProviderDiagnostic>,
) -> std::result::Result<Vec<NormalizedUsageRecord>, ProviderDiagnostic> {
    let source = source_identity_from_row(provider_surface, row);
    let period_start = period_start_field(&source.provider_surface, row)?;

    if let Some(breakdowns) = row.get("sourceBreakdowns").and_then(Value::as_array) {
        let mut records = Vec::new();
        for source_row in breakdowns {
            let child_source = source_identity_from_row(provider_surface, source_row);
            let child_period = period_start_field(&child_source.provider_surface, source_row)
                .unwrap_or_else(|_| period_start.clone());
            match normalize_single_source_record(
                &child_source,
                &child_period,
                source_row,
                diagnostics,
            ) {
                Ok(child_records) => records.extend(child_records),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
        if !records.is_empty() {
            return Ok(records);
        }
    }

    if source.provider_surface == "unknown" && is_aggregate_all_row(row) {
        return Err(ProviderDiagnostic {
            provider_surface: provider_surface.to_string(),
            code: "aggregate_all_source_ignored".to_string(),
            message: format!(
                "{provider_surface} row has agent:all with no per-source breakdown; ignoring"
            ),
        });
    }

    normalize_single_source_record(&source, &period_start, row, diagnostics)
}

fn normalize_single_source_record(
    source: &SourceIdentity,
    period_start: &str,
    row: &Value,
    diagnostics: &mut Vec<ProviderDiagnostic>,
) -> std::result::Result<Vec<NormalizedUsageRecord>, ProviderDiagnostic> {
    if let Some(breakdowns) = row.get("modelBreakdowns").and_then(Value::as_array) {
        let mut records = Vec::new();
        for model_row in breakdowns {
            match parse_claude_style_totals(&source.provider_surface, model_row) {
                Ok(raw_totals) => records.push(NormalizedUsageRecord {
                    source_identity: source.clone(),
                    period_start: period_start.to_string(),
                    model: optional_string(model_row, "modelName"),
                    raw_totals,
                    display_cost_usd: optional_f64(model_row, "cost"),
                    confidence: "local-log-derived".to_string(),
                }),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
        if !records.is_empty() {
            return Ok(records);
        }
    }

    if let Some(models) = row.get("models").and_then(Value::as_object) {
        let mut records = Vec::new();
        for (model, model_row) in models {
            match parse_openai_style_totals(&source.provider_surface, model_row) {
                Ok(raw_totals) => records.push(NormalizedUsageRecord {
                    source_identity: source.clone(),
                    period_start: period_start.to_string(),
                    model: Some(model.clone()),
                    raw_totals,
                    display_cost_usd: optional_f64(model_row, "costUSD")
                        .or_else(|| optional_f64(row, "costUSD")),
                    confidence: "local-log-derived".to_string(),
                }),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
        if !records.is_empty() {
            return Ok(records);
        }
    }

    let raw_totals = parse_row_totals_by_shape(&source.provider_surface, row)?;
    Ok(vec![NormalizedUsageRecord {
        source_identity: source.clone(),
        period_start: period_start.to_string(),
        model: first_string(row, "modelsUsed").or_else(|| optional_string(row, "model")),
        raw_totals,
        display_cost_usd: optional_f64(row, "totalCost").or_else(|| optional_f64(row, "costUSD")),
        confidence: "local-log-derived".to_string(),
    }])
}

fn parse_row_totals_by_shape(
    provider_surface: &str,
    value: &Value,
) -> std::result::Result<RawTokenTotals, ProviderDiagnostic> {
    let has_cache_read = value.get("cacheReadTokens").is_some();
    let has_cached_input = value.get("cachedInputTokens").is_some();

    if has_cache_read && has_cached_input {
        return Err(ProviderDiagnostic {
            provider_surface: provider_surface.to_string(),
            code: "ambiguous_token_shape".to_string(),
            message: format!(
                "{provider_surface} row has both cacheReadTokens and cachedInputTokens"
            ),
        });
    }

    if has_cache_read || value.get("cacheCreationTokens").is_some() {
        return parse_claude_style_totals(provider_surface, value);
    }

    if has_cached_input {
        return parse_openai_style_totals(provider_surface, value);
    }

    Err(ProviderDiagnostic {
        provider_surface: provider_surface.to_string(),
        code: "unsupported_token_shape".to_string(),
        message: format!("{provider_surface} row has no recognizable cache token fields"),
    })
}

fn parse_claude_style_totals(
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

fn parse_openai_style_totals(
    provider_surface: &str,
    value: &Value,
) -> std::result::Result<RawTokenTotals, ProviderDiagnostic> {
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

fn required_u64(
    provider_surface: &str,
    value: &Value,
    field: &str,
) -> std::result::Result<u64, ProviderDiagnostic> {
    optional_u64(value, field).ok_or_else(|| missing(provider_surface, field))
}

/// The daily row's period date. ccusage >= 20 renamed `date` to `period`
/// (same `YYYY-MM-DD` value); older helpers — and ccusage-codex, which has
/// not picked up the rename yet — still emit `date`. Prefer `date` so
/// existing helper versions stay byte-stable, accept `period` as fallback.
fn period_start_field(
    provider_surface: &str,
    row: &Value,
) -> std::result::Result<String, ProviderDiagnostic> {
    optional_string(row, "date")
        .or_else(|| optional_string(row, "period"))
        .ok_or_else(|| missing(provider_surface, "date/period"))
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
        let row = serde_json::json!({
            "inputTokens": 19_994_265,
            "outputTokens": 118_665,
            "cachedInputTokens": 19_074_688,
            "reasoningOutputTokens": 49_449
        });
        let totals = parse_openai_style_totals("codex", &row).unwrap();
        assert_eq!(totals.uncached_input, 919_577);
        assert_eq!(totals.cache_read, 19_074_688);

        let weights = EffectiveTokenWeights::default();
        let effective = totals.effective_tokens(weights);
        assert!(
            (1_610_000.0..1_611_000.0).contains(&effective),
            "got {effective}"
        );
    }

    #[test]
    fn ccusage_v20_period_field_is_accepted_for_claude_and_codex_rows() {
        let payload = serde_json::json!({
            "daily": [{
                "period": "2026-06-10",
                "agent": "claude",
                "inputTokens": 30_344,
                "outputTokens": 100_091,
                "cacheCreationTokens": 1_893_797,
                "cacheReadTokens": 9_813_957,
                "totalCost": 0,
                "modelsUsed": ["claude-fable-5"],
                "modelBreakdowns": [{
                    "modelName": "claude-fable-5",
                    "inputTokens": 30_477,
                    "outputTokens": 103_773,
                    "cacheCreationTokens": 1_897_540,
                    "cacheReadTokens": 10_761_204,
                    "cost": 0.0
                }]
            }]
        })
        .to_string();
        let batch = normalize_usage_json("claude-code", &payload).unwrap();
        assert!(!batch.records.is_empty());
        assert!(batch.records.iter().all(|r| r.period_start == "2026-06-10"));

        let codex_payload = serde_json::json!({
            "daily": [
                {
                    "period": "2026-06-10",
                    "models": {
                        "gpt-5": {
                            "inputTokens": 900,
                            "cachedInputTokens": 400,
                            "outputTokens": 250,
                            "reasoningOutputTokens": 50,
                            "costUSD": 0.42
                        }
                    }
                }
            ]
        })
        .to_string();
        let codex_batch = normalize_usage_json("codex", &codex_payload).unwrap();
        assert_eq!(codex_batch.records[0].period_start, "2026-06-10");
    }

    #[test]
    fn legacy_date_field_is_still_preferred_over_period() {
        let payload = serde_json::json!({
            "daily": [{
                "date": "2026-06-09",
                "period": "2026-06-10",
                "inputTokens": 1,
                "outputTokens": 1,
                "cacheCreationTokens": 0,
                "cacheReadTokens": 0
            }]
        })
        .to_string();
        let batch = normalize_usage_json("claude-code", &payload).unwrap();
        assert_eq!(batch.records[0].period_start, "2026-06-09");
    }

    #[test]
    fn claude_input_already_excludes_cache_reads() {
        let row = serde_json::json!({
            "inputTokens": 100,
            "outputTokens": 200,
            "cacheCreationTokens": 50,
            "cacheReadTokens": 1000
        });
        let totals = parse_claude_style_totals("claude-code", &row).unwrap();
        assert_eq!(totals.uncached_input, 100);
        assert_eq!(totals.cache_creation, 50);
        assert_eq!(totals.cache_read, 1000);
    }
}
