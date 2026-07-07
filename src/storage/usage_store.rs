use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use time::{format_description::well_known::Rfc3339, Date, OffsetDateTime};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedUsageEvent {
    pub provider_surface: String,
    pub provider_version: String,
    pub parser_version: String,
    pub command: String,
    pub source_surface: String,
    pub period_start: OffsetDateTime,
    pub observed_at: OffsetDateTime,
    pub bucket_at: OffsetDateTime,
    pub model: Option<String>,
    pub input_tokens: f64,
    pub output_tokens: f64,
    pub cache_creation_tokens: f64,
    pub cache_read_tokens: f64,
    pub reasoning_output_tokens: f64,
    pub effective_tokens: f64,
    pub total_tokens: f64,
    pub token_contract: String,
    pub cost_usd: Option<f64>,
    pub confidence: String,
    pub provider_delta_id: Option<String>,
}

/// Component sums for a day-window token-shape read (DaySummary/climate input).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AppliedShapeSums {
    /// Sum of raw input tokens.
    pub input_tokens: f64,
    /// Sum of raw output tokens.
    pub output_tokens: f64,
    /// Sum of cache creation tokens.
    pub cache_creation_tokens: f64,
    /// Sum of cache read tokens.
    pub cache_read_tokens: f64,
    /// Sum of reasoning output tokens.
    pub reasoning_output_tokens: f64,
    /// Sum of effective tokens (weighted composite).
    pub effective_tokens: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCursorUpdate {
    pub provider_surface: String,
    pub cursor_key: String,
    pub cursor_value: String,
    pub provider_version: String,
    pub parser_version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageLedgerRow {
    pub id: i64,
    pub event: NormalizedUsageEvent,
    pub cursor_update: ProviderCursorUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderDiagnostic {
    pub provider_surface: String,
    pub code: String,
    pub message: String,
    pub recorded_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderVersionInfo {
    pub provider_surface: String,
    pub provider_version: String,
    pub parser_version: String,
}

pub struct UsageStore {
    conn: Connection,
}

#[derive(Debug, Clone, PartialEq)]
struct SourceDaySnapshot {
    total_tokens: f64,
    identity_fingerprints: BTreeSet<String>,
}

struct SourceDaySnapshotComparison<'a> {
    previous: &'a BTreeMap<String, SourceDaySnapshot>,
    current: &'a BTreeMap<String, SourceDaySnapshot>,
}

enum SnapshotAttemptState {
    Complete {
        observed_at: OffsetDateTime,
    },
    Blocked {
        observed_at: OffsetDateTime,
        reason: Option<String>,
    },
    Missing,
}

impl NormalizedUsageEvent {
    pub fn for_test_at(period_start: OffsetDateTime, effective_tokens: f64) -> Self {
        Self {
            provider_surface: "claude-code".to_string(),
            provider_version: "test-provider".to_string(),
            parser_version: "test-parser".to_string(),
            command: "ccusage daily --json --offline".to_string(),
            source_surface: "daily".to_string(),
            period_start,
            observed_at: period_start,
            bucket_at: period_start,
            model: Some("test-model".to_string()),
            input_tokens: effective_tokens,
            output_tokens: 0.0,
            cache_creation_tokens: 0.0,
            cache_read_tokens: 0.0,
            reasoning_output_tokens: 0.0,
            effective_tokens,
            total_tokens: effective_tokens,
            token_contract: crate::usage::token_contract::TOKENMAXXING_TOTAL_V1.to_string(),
            cost_usd: None,
            confidence: "local-log-derived".to_string(),
            provider_delta_id: None,
        }
    }

    pub fn for_test_with_ignored_payloads(
        provider_surface: &str,
        _prompt: &str,
        _response: &str,
        _tool_payload: &str,
    ) -> Self {
        Self {
            provider_surface: provider_surface.to_string(),
            ..Self::for_test_at(OffsetDateTime::UNIX_EPOCH, 1.0)
        }
    }
}

impl UsageStore {
    pub fn open(path: &Path) -> crate::error::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    #[doc(hidden)]
    pub fn raw_connection_for_test(&self) -> &rusqlite::Connection {
        &self.conn
    }

    pub fn insert_event(&mut self, event: &NormalizedUsageEvent) -> crate::error::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO usage_events (
                provider_surface,
                provider_version,
                parser_version,
                command,
                source_surface,
                period_start,
                observed_at,
                bucket_at,
                period_date,
                model,
                input_tokens,
                output_tokens,
                cache_creation_tokens,
                cache_read_tokens,
                reasoning_output_tokens,
                effective_tokens,
                total_tokens,
                token_contract,
                cost_usd,
                confidence,
                applied_at,
                feedable
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
            )",
            params![
                event.provider_surface,
                event.provider_version,
                event.parser_version,
                event.command,
                event.source_surface,
                format_time(event.period_start)?,
                format_time(event.observed_at)?,
                format_time(event.bucket_at)?,
                event.period_start.date().to_string(),
                event.model,
                event.input_tokens,
                event.output_tokens,
                event.cache_creation_tokens,
                event.cache_read_tokens,
                event.reasoning_output_tokens,
                event.effective_tokens,
                event.total_tokens,
                event.token_contract,
                event.cost_usd,
                event.confidence,
                format_time(event.observed_at)?,
                1_i64,
            ],
        )?;
        add_lifetime_counter(&tx, event.effective_tokens)?;
        tx.commit()?;
        Ok(())
    }

    pub fn insert_unapplied_event_bucket(
        &mut self,
        event: &NormalizedUsageEvent,
        cursor_update: &ProviderCursorUpdate,
        bucket_index: usize,
        bucket_count: usize,
    ) -> crate::error::Result<i64> {
        let provider_delta_id = format!(
            "{}|{}|{}",
            cursor_update.provider_surface, cursor_update.cursor_key, cursor_update.cursor_value
        );
        let bucket_index_i64 = bucket_index as i64;
        let bucket_count_i64 = bucket_count as i64;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO usage_events (
                provider_surface,
                provider_version,
                parser_version,
                command,
                source_surface,
                period_start,
                observed_at,
                bucket_at,
                period_date,
                model,
                input_tokens,
                output_tokens,
                cache_creation_tokens,
                cache_read_tokens,
                reasoning_output_tokens,
                effective_tokens,
                total_tokens,
                token_contract,
                cost_usd,
                confidence,
                provider_delta_id,
                bucket_index,
                bucket_count,
                applied_at,
                feedable,
                provider_cursor_key,
                provider_cursor_value
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                ?19, ?20, ?21, ?22, ?23, NULL, ?24, ?25, ?26
            )",
            params![
                event.provider_surface,
                event.provider_version,
                event.parser_version,
                event.command,
                event.source_surface,
                format_time(event.period_start)?,
                format_time(event.observed_at)?,
                format_time(event.bucket_at)?,
                event.period_start.date().to_string(),
                event.model,
                event.input_tokens,
                event.output_tokens,
                event.cache_creation_tokens,
                event.cache_read_tokens,
                event.reasoning_output_tokens,
                event.effective_tokens,
                event.total_tokens,
                event.token_contract,
                event.cost_usd,
                event.confidence,
                provider_delta_id,
                bucket_index_i64,
                bucket_count_i64,
                1_i64,
                cursor_update.cursor_key,
                cursor_update.cursor_value,
            ],
        )?;
        let id: i64 = tx.query_row(
            "SELECT id FROM usage_events
             WHERE provider_delta_id = ?1 AND bucket_index = ?2",
            params![provider_delta_id, bucket_index_i64],
            |row| row.get(0),
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Writes historical rows as already-applied ledger entries and advances
    /// their source cursors, but does NOT increment the lifetime counter or
    /// stage anything for feeding. Used for first-contact history safety.
    pub fn seed_source_history(
        &mut self,
        events: &[(NormalizedUsageEvent, ProviderCursorUpdate)],
        diagnostic: Option<&ProviderDiagnostic>,
        seeded_at: OffsetDateTime,
    ) -> crate::error::Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        let seeded_at_text = format_time(seeded_at)?;
        let tx = self.conn.transaction()?;

        for (event, cursor_update) in events {
            let provider_delta_id = format!(
                "{}|{}|{}",
                cursor_update.provider_surface,
                cursor_update.cursor_key,
                cursor_update.cursor_value
            );

            tx.execute(
                "INSERT OR IGNORE INTO usage_events (
                    provider_surface, provider_version, parser_version, command, source_surface,
                    period_start, observed_at, bucket_at, period_date, model,
                    input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens,
                    reasoning_output_tokens, effective_tokens, total_tokens, token_contract,
                    cost_usd, confidence,
                    provider_delta_id, bucket_index, bucket_count, applied_at, feedable,
                    provider_cursor_key, provider_cursor_value
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                    ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                    ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27
                )",
                params![
                    event.provider_surface,
                    event.provider_version,
                    event.parser_version,
                    event.command,
                    event.source_surface,
                    format_time(event.period_start)?,
                    format_time(event.observed_at)?,
                    format_time(event.bucket_at)?,
                    event.period_start.date().to_string(),
                    event.model,
                    event.input_tokens,
                    event.output_tokens,
                    event.cache_creation_tokens,
                    event.cache_read_tokens,
                    event.reasoning_output_tokens,
                    event.effective_tokens,
                    event.total_tokens,
                    event.token_contract,
                    event.cost_usd,
                    event.confidence,
                    provider_delta_id,
                    0_i64,
                    1_i64,
                    &seeded_at_text,
                    0_i64,
                    cursor_update.cursor_key,
                    cursor_update.cursor_value,
                ],
            )?;

            upsert_provider_cursor(&tx, cursor_update, &seeded_at_text)?;
        }

        if let Some(d) = diagnostic {
            tx.execute(
                "INSERT INTO provider_diagnostics (
                    provider_surface, code, message, recorded_at
                ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    d.provider_surface,
                    d.code,
                    d.message,
                    format_time(d.recorded_at)?,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Compacts usage events whose `period_start` and `bucket_at` are both
    /// strictly less than `cutoff` into `daily_aggregates`. Rows with a NULL
    /// `applied_at` (unapplied events) are never touched.
    pub fn compact_before(&mut self, cutoff: OffsetDateTime) -> crate::error::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO daily_aggregates (
                provider_surface,
                period_date,
                source_surface,
                input_tokens,
                output_tokens,
                cache_creation_tokens,
                cache_read_tokens,
                reasoning_output_tokens,
                effective_tokens,
                cost_usd,
                event_count
            )
            SELECT
                provider_surface,
                period_date,
                source_surface,
                SUM(input_tokens),
                SUM(output_tokens),
                SUM(cache_creation_tokens),
                SUM(cache_read_tokens),
                SUM(reasoning_output_tokens),
                SUM(effective_tokens),
                SUM(COALESCE(cost_usd, 0.0)),
                COUNT(*)
            FROM usage_events
            WHERE period_start < ?1
              AND bucket_at < ?1
              AND applied_at IS NOT NULL
              AND feedable = 1
            GROUP BY provider_surface, period_date, source_surface
            ON CONFLICT(provider_surface, period_date, source_surface) DO UPDATE SET
                input_tokens = daily_aggregates.input_tokens + excluded.input_tokens,
                output_tokens = daily_aggregates.output_tokens + excluded.output_tokens,
                cache_creation_tokens = daily_aggregates.cache_creation_tokens + excluded.cache_creation_tokens,
                cache_read_tokens = daily_aggregates.cache_read_tokens + excluded.cache_read_tokens,
                reasoning_output_tokens = daily_aggregates.reasoning_output_tokens + excluded.reasoning_output_tokens,
                effective_tokens = daily_aggregates.effective_tokens + excluded.effective_tokens,
                cost_usd = daily_aggregates.cost_usd + excluded.cost_usd,
                event_count = daily_aggregates.event_count + excluded.event_count",
            params![format_time(cutoff)?],
        )?;
        tx.execute(
            "DELETE FROM usage_events
             WHERE period_start < ?1
               AND bucket_at < ?1
               AND applied_at IS NOT NULL
               AND feedable = 1",
            params![format_time(cutoff)?],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn recent_event_count(&self) -> crate::error::Result<u64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM usage_events WHERE feedable = 1",
                [],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn daily_aggregate_effective_tokens(
        &self,
        provider_surface: &str,
    ) -> crate::error::Result<f64> {
        self.conn
            .query_row(
                "SELECT COALESCE(SUM(effective_tokens), 0.0)
                 FROM daily_aggregates
                 WHERE provider_surface = ?1",
                params![provider_surface],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn lifetime_effective_tokens(&self) -> crate::error::Result<f64> {
        self.conn
            .query_row(
                "SELECT effective_tokens FROM lifetime_counters WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map(|value| value.unwrap_or(0.0))
            .map_err(Into::into)
    }

    pub fn set_provider_cursor(
        &self,
        provider_surface: &str,
        cursor_key: &str,
        cursor_value: &str,
        provider_version: &str,
        parser_version: &str,
    ) -> crate::error::Result<()> {
        self.conn.execute(
            "INSERT INTO provider_cursors (
                provider_surface,
                cursor_key,
                cursor_value,
                provider_version,
                parser_version,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(provider_surface, cursor_key) DO UPDATE SET
                cursor_value = excluded.cursor_value,
                provider_version = excluded.provider_version,
                parser_version = excluded.parser_version,
                updated_at = excluded.updated_at",
            params![
                provider_surface,
                cursor_key,
                cursor_value,
                provider_version,
                parser_version,
                format_time(OffsetDateTime::now_utc())?,
            ],
        )?;
        Ok(())
    }

    pub fn provider_cursor(
        &self,
        provider_surface: &str,
        cursor_key: &str,
    ) -> crate::error::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT cursor_value FROM provider_cursors
                 WHERE provider_surface = ?1 AND cursor_key = ?2",
                params![provider_surface, cursor_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn advance_cursors(
        &mut self,
        updates: Vec<ProviderCursorUpdate>,
        now: OffsetDateTime,
    ) -> crate::error::Result<()> {
        if updates.is_empty() {
            return Ok(());
        }
        let updated_at = format_time(now)?;
        let tx = self.conn.transaction()?;
        for update in &updates {
            upsert_provider_cursor(&tx, update, &updated_at)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn write_provider_snapshot_batch(
        &mut self,
        batch: &crate::usage::snapshot::ProviderSnapshotBatchInput,
        rows: &[crate::usage::snapshot::ProviderSnapshotRowInput],
        diagnostics: &[crate::usage::snapshot::ProviderSnapshotDiagnosticInput],
    ) -> crate::error::Result<crate::usage::snapshot::ProviderSnapshotWriteOutcome> {
        let tx = self.conn.transaction()?;
        let requested_days_json = provider_days_json(&batch.requested_provider_days)?;
        tx.execute(
            "INSERT INTO provider_snapshot_batches (
                collector_scope_id, collector_surface, command, token_contract,
                requested_provider_days_json, provider_version, parser_version,
                observed_at, completion_status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'complete')",
            params![
                batch.collector_scope_id,
                batch.collector_surface,
                batch.command,
                batch.token_contract,
                requested_days_json,
                batch.provider_version,
                batch.parser_version,
                format_time(batch.observed_at)?,
            ],
        )?;
        let batch_id = tx.last_insert_rowid();
        let mut complete_run_ids = Vec::new();
        let mut blocked_run_ids = Vec::new();

        for day in &batch.requested_provider_days {
            let day_rows = rows
                .iter()
                .filter(|row| row.provider_day == *day)
                .collect::<Vec<_>>();
            let day_diagnostics = diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.provider_day == Some(*day))
                .collect::<Vec<_>>();
            let replacement_scope_id = day_rows
                .first()
                .map(|row| row.replacement_scope_id.as_str())
                .or_else(|| {
                    day_diagnostics
                        .first()
                        .and_then(|diagnostic| diagnostic.replacement_scope_id.as_deref())
                })
                .unwrap_or(batch.collector_scope_id.as_str());
            let completion_status = if day_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.diagnostic_kind == "run_blocked")
            {
                "blocked"
            } else {
                "complete"
            };
            tx.execute(
                "INSERT INTO provider_snapshot_runs (
                    batch_id, replacement_scope_id, collector_scope_id, collector_surface,
                    command, token_contract, provider_day, provider_version, parser_version,
                    observed_at, completion_status, reason_code
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    batch_id,
                    replacement_scope_id,
                    batch.collector_scope_id,
                    batch.collector_surface,
                    batch.command,
                    batch.token_contract,
                    day.to_string(),
                    batch.provider_version,
                    batch.parser_version,
                    format_time(batch.observed_at)?,
                    completion_status,
                    day_diagnostics
                        .first()
                        .map(|diagnostic| diagnostic.reason_code.as_str()),
                ],
            )?;
            let run_id = tx.last_insert_rowid();
            if completion_status == "blocked" {
                blocked_run_ids.push(run_id);
                insert_snapshot_diagnostics(&tx, batch_id, Some(run_id), &day_diagnostics)?;
                continue;
            }

            let previous =
                canonical_visible_source_day_snapshots(&tx, &batch.token_contract, *day)?;
            complete_run_ids.push(run_id);
            if day_rows.is_empty() {
                supersede_previous_snapshot_rows_for_day(&tx, &batch.token_contract, *day)?;
            } else {
                supersede_previous_snapshot_rows(
                    &tx,
                    replacement_scope_id,
                    &batch.token_contract,
                    *day,
                )?;
            }
            insert_snapshot_rows(&tx, run_id, &day_rows, batch.observed_at)?;
            refresh_canonical_collectors(
                &tx,
                replacement_scope_id,
                &batch.token_contract,
                *day,
                &day_rows,
                batch.observed_at,
            )?;
            insert_snapshot_diagnostics(&tx, batch_id, Some(run_id), &day_diagnostics)?;
            let current = canonical_visible_source_day_snapshots(&tx, &batch.token_contract, *day)?;
            let comparison = SourceDaySnapshotComparison { previous: &previous, current: &current };
            record_snapshot_corrections(&tx, batch_id, run_id, batch, *day, &day_rows, comparison)?;
        }

        tx.commit()?;
        Ok(crate::usage::snapshot::ProviderSnapshotWriteOutcome {
            batch_id,
            complete_run_ids,
            blocked_run_ids,
        })
    }

    pub fn record_snapshot_failure(
        &mut self,
        diagnostic: &crate::usage::snapshot::ProviderSnapshotDiagnosticInput,
    ) -> crate::error::Result<()> {
        let requested_days_json = provider_days_json(&diagnostic.requested_provider_days)?;
        self.conn.execute(
            "INSERT INTO provider_snapshot_diagnostics (
                diagnostic_kind, collector_scope_id, replacement_scope_id,
                requested_provider_days_json, provider_day, reason_code, message,
                batch_id, run_id, recorded_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, ?8)",
            params![
                diagnostic.diagnostic_kind,
                diagnostic.collector_scope_id,
                diagnostic.replacement_scope_id,
                requested_days_json,
                diagnostic.provider_day.map(|day| day.to_string()),
                diagnostic.reason_code,
                diagnostic.message,
                format_time(diagnostic.observed_at)?,
            ],
        )?;
        Ok(())
    }

    pub fn snapshot_totals_for_provider_day(
        &self,
        day: Date,
    ) -> crate::error::Result<
        crate::usage::snapshot::SnapshotResult<crate::usage::snapshot::DayTotals>,
    > {
        let state = self.snapshot_state_for_provider_day(day)?;
        let provider_day = day;
        Ok(match state {
            SnapshotAttemptState::Missing => crate::usage::snapshot::SnapshotResult {
                state: crate::usage::snapshot::SnapshotState::Missing,
                value: None,
                provider_day,
                observed_at: None,
                reason: Some("not_polled".to_string()),
            },
            SnapshotAttemptState::Complete { observed_at } => {
                crate::usage::snapshot::SnapshotResult {
                    state: crate::usage::snapshot::SnapshotState::Current,
                    value: Some(crate::usage::snapshot::DayTotals {
                        total_tokens: self.active_snapshot_total_for_day(day)?,
                    }),
                    provider_day,
                    observed_at: Some(observed_at),
                    reason: None,
                }
            }
            SnapshotAttemptState::Blocked { observed_at, reason } => {
                if self.has_complete_snapshot_for_day(day)? {
                    crate::usage::snapshot::SnapshotResult {
                        state: crate::usage::snapshot::SnapshotState::Stale,
                        value: Some(crate::usage::snapshot::DayTotals {
                            total_tokens: self.active_snapshot_total_for_day(day)?,
                        }),
                        provider_day,
                        observed_at: Some(observed_at),
                        reason,
                    }
                } else {
                    crate::usage::snapshot::SnapshotResult {
                        state: crate::usage::snapshot::SnapshotState::Blocked,
                        value: None,
                        provider_day,
                        observed_at: Some(observed_at),
                        reason,
                    }
                }
            }
        })
    }

    pub fn snapshot_totals_by_source_for_provider_day(
        &self,
        day: Date,
    ) -> crate::error::Result<
        crate::usage::snapshot::SnapshotResult<crate::usage::snapshot::SourceTotals>,
    > {
        let totals = self.snapshot_totals_for_provider_day(day)?;
        let value = if totals.value.is_some() {
            Some(crate::usage::snapshot::SourceTotals {
                sources: self.active_snapshot_source_totals_for_day(day)?,
            })
        } else {
            None
        };
        Ok(crate::usage::snapshot::SnapshotResult {
            state: totals.state,
            value,
            provider_day: totals.provider_day,
            observed_at: totals.observed_at,
            reason: totals.reason,
        })
    }

    pub fn snapshot_token_history_for_provider_days(
        &self,
        days: &[Date],
    ) -> crate::error::Result<
        Vec<crate::usage::snapshot::SnapshotResult<crate::usage::snapshot::DayTotals>>,
    > {
        days.iter()
            .map(|day| self.snapshot_totals_for_provider_day(*day))
            .collect()
    }

    pub fn snapshot_health_for_provider_day(
        &self,
        day: Date,
        recent_accepted: &[(String, f64)],
    ) -> crate::error::Result<Vec<crate::usage::snapshot::SourceSnapshotHealth>> {
        let snapshot = self.snapshot_totals_by_source_for_provider_day(day)?;
        let mut recent_by_source = recent_accepted
            .iter()
            .cloned()
            .collect::<BTreeMap<String, f64>>();
        let mut rows = Vec::new();

        if let Some(totals) = snapshot.value {
            for source in totals.sources {
                let recent = recent_by_source
                    .remove(&source.accounting_source)
                    .unwrap_or(0.0);
                rows.push(crate::usage::snapshot::SourceSnapshotHealth {
                    display_name: source.accounting_source.clone(),
                    accounting_source: source.accounting_source,
                    snapshot_state: snapshot.state,
                    snapshot_total_tokens: Some(source.total_tokens),
                    recent_accepted_tokens: recent,
                    reason: snapshot.reason.clone(),
                });
            }
        }

        for (source, recent) in recent_by_source {
            rows.push(crate::usage::snapshot::SourceSnapshotHealth {
                accounting_source: source.clone(),
                display_name: source,
                snapshot_state: snapshot.state,
                snapshot_total_tokens: None,
                recent_accepted_tokens: recent,
                reason: snapshot.reason.clone(),
            });
        }

        Ok(rows)
    }

    fn snapshot_state_for_provider_day(
        &self,
        day: Date,
    ) -> crate::error::Result<SnapshotAttemptState> {
        let mut stmt = self.conn.prepare(
            "SELECT observed_at, completion_status, reason_code
             FROM (
                SELECT observed_at, completion_status, reason_code, id, 0 AS attempt_kind
                FROM provider_snapshot_runs
                WHERE token_contract = ?1 AND provider_day = ?2
                UNION ALL
                SELECT recorded_at AS observed_at, 'blocked' AS completion_status, reason_code, id, 1 AS attempt_kind
                FROM provider_snapshot_diagnostics
                WHERE diagnostic_kind = 'run_blocked' AND provider_day = ?2
             )
             ORDER BY observed_at DESC, attempt_kind DESC, id DESC
             LIMIT 1",
        )?;
        let row = stmt
            .query_row(
                params![
                    crate::usage::token_contract::TOKENMAXXING_TOTAL_V1,
                    day.to_string()
                ],
                |row| {
                    let observed_at: String = row.get(0)?;
                    let completion_status: String = row.get(1)?;
                    let reason: Option<String> = row.get(2)?;
                    Ok((observed_at, completion_status, reason))
                },
            )
            .optional()?;

        let Some((observed_at, completion_status, reason)) = row else {
            return Ok(SnapshotAttemptState::Missing);
        };
        let observed_at = parse_time_for_sql(&observed_at)?;
        if completion_status == "complete" {
            Ok(SnapshotAttemptState::Complete { observed_at })
        } else {
            Ok(SnapshotAttemptState::Blocked { observed_at, reason })
        }
    }

    fn has_complete_snapshot_for_day(&self, day: Date) -> crate::error::Result<bool> {
        self.conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM provider_snapshot_runs
                    WHERE token_contract = ?1
                      AND provider_day = ?2
                      AND completion_status = 'complete'
                )",
                params![
                    crate::usage::token_contract::TOKENMAXXING_TOTAL_V1,
                    day.to_string()
                ],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .map_err(Into::into)
    }

    fn active_snapshot_total_for_day(&self, day: Date) -> crate::error::Result<f64> {
        self.conn
            .query_row(
                "SELECT COALESCE(SUM(r.total_tokens), 0.0)
                 FROM provider_snapshot_rows AS r
                 JOIN provider_canonical_collectors AS c
                   ON c.token_contract = r.token_contract
                  AND c.accounting_source = r.accounting_source
                  AND c.provider_day = r.provider_day
                  AND c.replacement_scope_id = r.replacement_scope_id
                 WHERE r.token_contract = ?1
                   AND r.provider_day = ?2
                   AND r.status = 'active'",
                params![
                    crate::usage::token_contract::TOKENMAXXING_TOTAL_V1,
                    day.to_string()
                ],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn active_snapshot_source_totals_for_day(
        &self,
        day: Date,
    ) -> crate::error::Result<Vec<crate::usage::snapshot::SourceTotal>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.accounting_source, COALESCE(SUM(r.total_tokens), 0.0) AS total_tokens
             FROM provider_snapshot_rows AS r
             JOIN provider_canonical_collectors AS c
               ON c.token_contract = r.token_contract
              AND c.accounting_source = r.accounting_source
              AND c.provider_day = r.provider_day
              AND c.replacement_scope_id = r.replacement_scope_id
             WHERE r.token_contract = ?1
               AND r.provider_day = ?2
               AND r.status = 'active'
             GROUP BY r.accounting_source
             ORDER BY r.accounting_source",
        )?;
        let rows = stmt
            .query_map(
                params![
                    crate::usage::token_contract::TOKENMAXXING_TOTAL_V1,
                    day.to_string()
                ],
                |row| {
                    Ok(crate::usage::snapshot::SourceTotal {
                        accounting_source: row.get(0)?,
                        total_tokens: row.get(1)?,
                    })
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn is_token_contract_active(&self, contract: &str) -> crate::error::Result<bool> {
        self.conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM token_contract_state WHERE token_contract = ?1
                )",
                params![contract],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .map_err(Into::into)
    }

    pub fn mark_token_contract_active(
        &self,
        contract: &str,
        now: OffsetDateTime,
    ) -> crate::error::Result<()> {
        self.conn.execute(
            "INSERT INTO token_contract_state (token_contract, activated_at)
             VALUES (?1, ?2)
             ON CONFLICT(token_contract) DO UPDATE SET activated_at = excluded.activated_at",
            params![contract, format_time(now)?],
        )?;
        Ok(())
    }

    /// Newest `provider_cursors.updated_at` for one provider surface — the
    /// discontinuity guard's per-provider "last fed" instant. `None` means
    /// the surface has never had a cursor (first contact). MAX is computed
    /// lexically on the stored RFC3339 text; all writers share `format_time`,
    /// so orderings can differ only within a single second, which the
    /// guard's whole-day factor ignores.
    pub fn latest_cursor_updated_at(
        &self,
        provider_surface: &str,
    ) -> crate::error::Result<Option<OffsetDateTime>> {
        self.conn
            .query_row(
                "SELECT MAX(updated_at) FROM provider_cursors
                 WHERE provider_surface = ?1
                   AND cursor_key NOT LIKE 'helper_version::%'",
                params![provider_surface],
                |row| {
                    let raw: Option<String> = row.get(0)?;
                    raw.as_deref().map(parse_time_for_sql).transpose()
                },
            )
            .map_err(Into::into)
    }

    /// Refuse one provider surface's poll: advance its cursors WITHOUT
    /// staging any ledger rows and persist the refusal diagnostic, all in
    /// ONE transaction — a crash between separate writes would discard
    /// tokens with no record. Refused tokens are never retro-fed.
    pub fn refuse_poll_discontinuity(
        &mut self,
        updates: Vec<ProviderCursorUpdate>,
        diagnostic: &ProviderDiagnostic,
        now: OffsetDateTime,
    ) -> crate::error::Result<()> {
        let updated_at = format_time(now)?;
        let recorded_at = format_time(diagnostic.recorded_at)?;
        let tx = self.conn.transaction()?;
        for update in &updates {
            upsert_provider_cursor(&tx, update, &updated_at)?;
        }
        tx.execute(
            "INSERT INTO provider_diagnostics (
                provider_surface,
                code,
                message,
                recorded_at
            ) VALUES (?1, ?2, ?3, ?4)",
            params![
                diagnostic.provider_surface,
                diagnostic.code,
                diagnostic.message,
                recorded_at,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn insert_diagnostic(&self, diagnostic: &ProviderDiagnostic) -> crate::error::Result<()> {
        self.conn.execute(
            "INSERT INTO provider_diagnostics (
                provider_surface,
                code,
                message,
                recorded_at
            ) VALUES (?1, ?2, ?3, ?4)",
            params![
                diagnostic.provider_surface,
                diagnostic.code,
                diagnostic.message,
                format_time(diagnostic.recorded_at)?,
            ],
        )?;
        Ok(())
    }

    pub fn events_within(
        &self,
        duration: time::Duration,
        now: OffsetDateTime,
    ) -> crate::error::Result<Vec<NormalizedUsageEvent>> {
        let cutoff = format_time(now - duration)?;
        let mut stmt = self.conn.prepare(
            "SELECT
                provider_surface,
                provider_version,
                parser_version,
                command,
                source_surface,
                period_start,
                observed_at,
                bucket_at,
                model,
                input_tokens,
                output_tokens,
                cache_creation_tokens,
                cache_read_tokens,
                reasoning_output_tokens,
                effective_tokens,
                total_tokens,
                token_contract,
                cost_usd,
                confidence,
                provider_delta_id
             FROM usage_events
             WHERE observed_at >= ?1
               AND feedable = 1
             ORDER BY observed_at DESC, id DESC",
        )?;
        let events = stmt
            .query_map(params![cutoff], |row| {
                let period_start: String = row.get(5)?;
                let observed_at: String = row.get(6)?;
                let bucket_at: String = row.get(7)?;
                Ok(NormalizedUsageEvent {
                    provider_surface: row.get(0)?,
                    provider_version: row.get(1)?,
                    parser_version: row.get(2)?,
                    command: row.get(3)?,
                    source_surface: row.get(4)?,
                    period_start: parse_time_for_sql(&period_start)?,
                    observed_at: parse_time_for_sql(&observed_at)?,
                    bucket_at: parse_time_for_sql(&bucket_at)?,
                    model: row.get(8)?,
                    input_tokens: row.get(9)?,
                    output_tokens: row.get(10)?,
                    cache_creation_tokens: row.get(11)?,
                    cache_read_tokens: row.get(12)?,
                    reasoning_output_tokens: row.get(13)?,
                    effective_tokens: row.get(14)?,
                    total_tokens: row.get(15)?,
                    token_contract: row.get(16)?,
                    cost_usd: row.get(17)?,
                    confidence: row.get(18)?,
                    provider_delta_id: row.get(19)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(events)
    }

    pub fn recent_events(&self, limit: u32) -> crate::error::Result<Vec<NormalizedUsageEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                provider_surface,
                provider_version,
                parser_version,
                command,
                source_surface,
                period_start,
                observed_at,
                bucket_at,
                model,
                input_tokens,
                output_tokens,
                cache_creation_tokens,
                cache_read_tokens,
                reasoning_output_tokens,
                effective_tokens,
                total_tokens,
                token_contract,
                cost_usd,
                confidence,
                provider_delta_id
             FROM usage_events
             WHERE feedable = 1
             ORDER BY observed_at DESC, id DESC
             LIMIT ?1",
        )?;
        let events = stmt
            .query_map(params![limit], |row| {
                let period_start: String = row.get(5)?;
                let observed_at: String = row.get(6)?;
                let bucket_at: String = row.get(7)?;
                Ok(NormalizedUsageEvent {
                    provider_surface: row.get(0)?,
                    provider_version: row.get(1)?,
                    parser_version: row.get(2)?,
                    command: row.get(3)?,
                    source_surface: row.get(4)?,
                    period_start: parse_time_for_sql(&period_start)?,
                    observed_at: parse_time_for_sql(&observed_at)?,
                    bucket_at: parse_time_for_sql(&bucket_at)?,
                    model: row.get(8)?,
                    input_tokens: row.get(9)?,
                    output_tokens: row.get(10)?,
                    cache_creation_tokens: row.get(11)?,
                    cache_read_tokens: row.get(12)?,
                    reasoning_output_tokens: row.get(13)?,
                    effective_tokens: row.get(14)?,
                    total_tokens: row.get(15)?,
                    token_contract: row.get(16)?,
                    cost_usd: row.get(17)?,
                    confidence: row.get(18)?,
                    provider_delta_id: row.get(19)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(events)
    }

    pub fn unapplied_events(&self, limit: u32) -> crate::error::Result<Vec<UsageLedgerRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                id,
                provider_surface,
                provider_version,
                parser_version,
                command,
                source_surface,
                period_start,
                observed_at,
                bucket_at,
                model,
                input_tokens,
                output_tokens,
                cache_creation_tokens,
                cache_read_tokens,
                reasoning_output_tokens,
                effective_tokens,
                total_tokens,
                token_contract,
                cost_usd,
                confidence,
                provider_cursor_key,
                provider_cursor_value,
                provider_delta_id
             FROM usage_events
             WHERE applied_at IS NULL
               AND feedable = 1
             ORDER BY bucket_at ASC, id ASC
             LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit], |row| {
                let period_start: String = row.get(6)?;
                let observed_at: String = row.get(7)?;
                let bucket_at: String = row.get(8)?;
                let provider_surface: String = row.get(1)?;
                let provider_version: String = row.get(2)?;
                let parser_version: String = row.get(3)?;
                let cursor_key: Option<String> = row.get(20)?;
                let cursor_value: Option<String> = row.get(21)?;
                let event = NormalizedUsageEvent {
                    provider_surface: provider_surface.clone(),
                    provider_version: provider_version.clone(),
                    parser_version: parser_version.clone(),
                    command: row.get(4)?,
                    source_surface: row.get(5)?,
                    period_start: parse_time_for_sql(&period_start)?,
                    observed_at: parse_time_for_sql(&observed_at)?,
                    bucket_at: parse_time_for_sql(&bucket_at)?,
                    model: row.get(9)?,
                    input_tokens: row.get(10)?,
                    output_tokens: row.get(11)?,
                    cache_creation_tokens: row.get(12)?,
                    cache_read_tokens: row.get(13)?,
                    reasoning_output_tokens: row.get(14)?,
                    effective_tokens: row.get(15)?,
                    total_tokens: row.get(16)?,
                    token_contract: row.get(17)?,
                    cost_usd: row.get(18)?,
                    confidence: row.get(19)?,
                    provider_delta_id: row.get(22)?,
                };
                let cursor_update = ProviderCursorUpdate {
                    provider_surface,
                    cursor_key: cursor_key.unwrap_or_default(),
                    cursor_value: cursor_value.unwrap_or_default(),
                    provider_version,
                    parser_version,
                };
                Ok(UsageLedgerRow { id: row.get(0)?, event, cursor_update })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn mark_events_applied_and_advance_cursors(
        &mut self,
        event_ids: &[i64],
        applied_at: OffsetDateTime,
    ) -> crate::error::Result<()> {
        if event_ids.is_empty() {
            return Ok(());
        }
        let applied_at_text = format_time(applied_at)?;
        let tx = self.conn.transaction()?;
        let placeholders = event_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        // Load only rows that are currently unapplied; advancing cursors and
        // counters for already-applied rows would double-count.
        let mut pending_updates: Vec<ProviderCursorUpdate> = Vec::new();
        let mut pending_effective_tokens: Vec<f64> = Vec::new();
        {
            let select_sql = format!(
                "SELECT
                    provider_surface,
                    provider_version,
                    parser_version,
                    provider_cursor_key,
                    provider_cursor_value,
                    effective_tokens
                 FROM usage_events
                 WHERE id IN ({placeholders})
                   AND applied_at IS NULL
                   AND feedable = 1"
            );
            let mut stmt = tx.prepare(&select_sql)?;
            let rows = stmt.query_map(
                rusqlite::params_from_iter(event_ids.iter().copied()),
                |row| {
                    let cursor_key: Option<String> = row.get(3)?;
                    let cursor_value: Option<String> = row.get(4)?;
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        cursor_key,
                        cursor_value,
                        row.get::<_, f64>(5)?,
                    ))
                },
            )?;
            for row in rows {
                let (provider_surface, provider_version, parser_version, key, value, effective) =
                    row?;
                pending_effective_tokens.push(effective);
                if let (Some(cursor_key), Some(cursor_value)) = (key, value) {
                    pending_updates.push(ProviderCursorUpdate {
                        provider_surface,
                        cursor_key,
                        cursor_value,
                        provider_version,
                        parser_version,
                    });
                }
            }
        }
        let update_sql = format!(
            "UPDATE usage_events
             SET applied_at = ?
             WHERE id IN ({placeholders})
               AND applied_at IS NULL
               AND feedable = 1"
        );
        let mut update_params: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(applied_at_text.clone())];
        for id in event_ids {
            update_params.push(Box::new(*id));
        }
        tx.execute(
            &update_sql,
            rusqlite::params_from_iter(update_params.iter().map(|b| b.as_ref())),
        )?;
        for update in &pending_updates {
            upsert_provider_cursor(&tx, update, &applied_at_text)?;
        }
        for effective in pending_effective_tokens {
            if effective != 0.0 {
                add_lifetime_counter(&tx, effective)?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn recent_diagnostics(&self, limit: u32) -> crate::error::Result<Vec<ProviderDiagnostic>> {
        let mut stmt = self.conn.prepare(
            "SELECT provider_surface, code, message, recorded_at
             FROM provider_diagnostics
             ORDER BY recorded_at DESC, id DESC
             LIMIT ?1",
        )?;
        let diagnostics = stmt
            .query_map(params![limit], |row| {
                let recorded_at: String = row.get(3)?;
                Ok(ProviderDiagnostic {
                    provider_surface: row.get(0)?,
                    code: row.get(1)?,
                    message: row.get(2)?,
                    recorded_at: parse_time_for_sql(&recorded_at)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(diagnostics)
    }

    pub fn provider_versions(&self) -> crate::error::Result<Vec<ProviderVersionInfo>> {
        let mut stmt = self.conn.prepare(
            "SELECT provider_surface, provider_version, parser_version
             FROM provider_cursors
             GROUP BY provider_surface, provider_version, parser_version
             ORDER BY provider_surface",
        )?;
        let versions = stmt
            .query_map([], |row| {
                Ok(ProviderVersionInfo {
                    provider_surface: row.get(0)?,
                    provider_version: row.get(1)?,
                    parser_version: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(versions)
    }

    /// Today's applied effective tokens on the canonical local-day axis
    /// (local day of bucket_at). Half-open [local midnight, local midnight+1d)
    /// so late-night rows never double-count across the boundary.
    pub fn today_effective_tokens(
        &self,
        now: OffsetDateTime,
        mapper: crate::storage::day_axis::LocalDayMapper,
    ) -> crate::error::Result<f64> {
        let today = mapper.local_date(now);
        let start = mapper
            .local_day_start(today)
            .to_offset(time::UtcOffset::UTC);
        let end = mapper
            .local_day_start(today + time::Duration::days(1))
            .to_offset(time::UtcOffset::UTC);
        self.applied_effective_tokens_between(start, end)
    }

    /// Sum effective tokens per provider_surface for events whose bucket_at
    /// falls in the closed interval `[start, end]`. Uses a single SQL
    /// aggregate so callers don't have to fetch + filter rows in memory
    /// (the watch view previously clipped the slice to 500 rows and silently
    /// undercounted today).
    ///
    /// Inclusive on both sides on purpose: RFC3339 timestamps without
    /// fractional seconds (`...:00Z`) lexically sort AFTER values with
    /// fractional seconds (`...:00.001Z`) because `Z` > `.`. A half-open
    /// `< end` filter would silently drop events whose bucket_at lands
    /// exactly at the caller's "now".
    pub fn token_totals_by_source_between(
        &self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> crate::error::Result<Vec<(String, f64)>> {
        let start_text = format_time(start)?;
        let end_text = format_time(end)?;
        let mut stmt = self.conn.prepare(
            "SELECT provider_surface, COALESCE(SUM(effective_tokens), 0.0) AS total
             FROM usage_events
             WHERE bucket_at >= ?1
               AND bucket_at <= ?2
               AND feedable = 1
             GROUP BY provider_surface
             ORDER BY provider_surface",
        )?;
        let rows = stmt
            .query_map(params![start_text, end_text], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Applied-only effective-token sum over the half-open bucket_at window
    /// `[start, end)`.
    ///
    /// Caller must ensure `start`/`end` share subsecond precision with stored
    /// `bucket_at` values (whole seconds / 10-minute-floored). Mixed fractional
    /// / whole-second bounds can misorder under lexical RFC3339 comparison.
    pub fn applied_effective_tokens_between(
        &self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> crate::error::Result<f64> {
        self.conn
            .query_row(
                "SELECT COALESCE(SUM(effective_tokens), 0.0)
                 FROM usage_events
                 WHERE applied_at IS NOT NULL
                   AND feedable = 1
                   AND bucket_at >= ?1
                   AND bucket_at < ?2",
                params![format_time(start)?, format_time(end)?],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn canonical_total_tokens_between(
        &self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> crate::error::Result<f64> {
        self.conn
            .query_row(
                "SELECT COALESCE(SUM(total_tokens), 0.0)
                 FROM usage_events
                 WHERE applied_at IS NOT NULL
                   AND feedable = 1
                   AND token_contract = ?1
                   AND bucket_at >= ?2
                   AND bucket_at < ?3",
                params![
                    crate::usage::token_contract::TOKENMAXXING_TOTAL_V1,
                    format_time(start)?,
                    format_time(end)?,
                ],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn canonical_total_tokens_by_source_between(
        &self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> crate::error::Result<Vec<(String, f64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT provider_surface, COALESCE(SUM(total_tokens), 0.0)
             FROM usage_events
             WHERE applied_at IS NOT NULL
               AND feedable = 1
               AND token_contract = ?1
               AND bucket_at >= ?2
               AND bucket_at < ?3
             GROUP BY provider_surface
             ORDER BY provider_surface",
        )?;
        let rows = stmt
            .query_map(
                params![
                    crate::usage::token_contract::TOKENMAXXING_TOTAL_V1,
                    format_time(start)?,
                    format_time(end)?,
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Applied-only per-source effective sums over the half-open bucket_at
    /// window `[start, end)`. DayContext's yesterday source mix; the
    /// closed-interval `token_totals_by_source_between` variant serves the
    /// today panel on the same feedable-only ledger.
    pub fn applied_effective_tokens_by_source_between(
        &self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> crate::error::Result<Vec<(String, f64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT provider_surface, COALESCE(SUM(effective_tokens), 0.0)
             FROM usage_events
             WHERE applied_at IS NOT NULL
               AND feedable = 1
               AND bucket_at >= ?1
               AND bucket_at < ?2
             GROUP BY provider_surface
             ORDER BY provider_surface",
        )?;
        let rows = stmt
            .query_map(params![format_time(start)?, format_time(end)?], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Applied-only bucket sums over `[start, end)`, ascending.
    ///
    /// Groups by distinct `bucket_at` value (not by 10-minute truncation).
    ///
    /// Caller must ensure `start`/`end` share subsecond precision with stored
    /// `bucket_at` values (whole seconds / 10-minute-floored). Mixed fractional
    /// / whole-second bounds can misorder under lexical RFC3339 comparison.
    pub fn applied_bucket_sums_between(
        &self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> crate::error::Result<Vec<(OffsetDateTime, f64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT bucket_at, SUM(effective_tokens)
             FROM usage_events
             WHERE applied_at IS NOT NULL
               AND feedable = 1
               AND bucket_at >= ?1
               AND bucket_at < ?2
             GROUP BY bucket_at
             ORDER BY bucket_at ASC",
        )?;
        let rows = stmt
            .query_map(params![format_time(start)?, format_time(end)?], |row| {
                let at: String = row.get(0)?;
                Ok((parse_time_for_sql(&at)?, row.get::<_, f64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Applied-only token-shape component sums over `[start, end)`.
    ///
    /// Caller must ensure `start`/`end` share subsecond precision with stored
    /// `bucket_at` values (whole seconds / 10-minute-floored). Mixed fractional
    /// / whole-second bounds can misorder under lexical RFC3339 comparison.
    pub fn applied_token_shape_between(
        &self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> crate::error::Result<AppliedShapeSums> {
        self.conn
            .query_row(
                "SELECT
                    COALESCE(SUM(input_tokens), 0.0),
                    COALESCE(SUM(output_tokens), 0.0),
                    COALESCE(SUM(cache_creation_tokens), 0.0),
                    COALESCE(SUM(cache_read_tokens), 0.0),
                    COALESCE(SUM(reasoning_output_tokens), 0.0),
                    COALESCE(SUM(effective_tokens), 0.0)
                 FROM usage_events
                 WHERE applied_at IS NOT NULL
                   AND feedable = 1
                   AND bucket_at >= ?1
                   AND bucket_at < ?2",
                params![format_time(start)?, format_time(end)?],
                |row| {
                    Ok(AppliedShapeSums {
                        input_tokens: row.get(0)?,
                        output_tokens: row.get(1)?,
                        cache_creation_tokens: row.get(2)?,
                        cache_read_tokens: row.get(3)?,
                        reasoning_output_tokens: row.get(4)?,
                        effective_tokens: row.get(5)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    /// Newest applied bucket_at, if any. No upper bound: future-dated rows
    /// (clock set backwards) surface here, which is the fail-awake rule.
    pub fn latest_applied_bucket_at(&self) -> crate::error::Result<Option<OffsetDateTime>> {
        let max: Option<String> = self.conn.query_row(
            "SELECT MAX(bucket_at) FROM usage_events
             WHERE applied_at IS NOT NULL
               AND feedable = 1",
            [],
            |row| row.get(0),
        )?;
        max.map(|s| parse_time_for_sql(&s).map_err(Into::into))
            .transpose()
    }

    /// When the most recent apply happened (MAX(applied_at)) — the wake
    /// instant for resume easing; bucket_at is 10-minute-floored and too
    /// coarse for an 8-second ease.
    pub fn latest_applied_marked_at(&self) -> crate::error::Result<Option<OffsetDateTime>> {
        let max: Option<String> = self.conn.query_row(
            "SELECT MAX(applied_at) FROM usage_events
             WHERE applied_at IS NOT NULL
               AND feedable = 1",
            [],
            |row| row.get(0),
        )?;
        max.map(|s| parse_time_for_sql(&s).map_err(Into::into))
            .transpose()
    }

    /// Newest applied bucket_at strictly before `at` (wake-resume easing).
    ///
    /// Caller must ensure `at` shares subsecond precision with stored
    /// `bucket_at` values (whole seconds / 10-minute-floored). Mixed fractional
    /// / whole-second bounds can misorder under lexical RFC3339 comparison.
    pub fn latest_applied_bucket_at_before(
        &self,
        at: OffsetDateTime,
    ) -> crate::error::Result<Option<OffsetDateTime>> {
        let max: Option<String> = self.conn.query_row(
            "SELECT MAX(bucket_at) FROM usage_events
             WHERE applied_at IS NOT NULL
               AND feedable = 1
               AND bucket_at < ?1",
            params![format_time(at)?],
            |row| row.get(0),
        )?;
        max.map(|s| parse_time_for_sql(&s).map_err(Into::into))
            .transpose()
    }

    /// Whether any applied, feedable ledger row exists.
    /// Newborn sleep gate.
    pub fn has_any_applied_events(&self) -> crate::error::Result<bool> {
        self.conn
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM usage_events
                    WHERE applied_at IS NOT NULL
                      AND feedable = 1
                )",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|v| v != 0)
            .map_err(Into::into)
    }

    /// Effective tokens per local day for the trailing 7 local days (oldest
    /// first, today last), on the canonical bucket_at axis, applied rows only.
    /// daily_aggregates is deliberately NOT consulted: compaction's cutoff is
    /// 90 days, so aggregate rows cannot occur inside a 7-day window.
    pub fn seven_day_token_history(
        &self,
        now: OffsetDateTime,
        mapper: crate::storage::day_axis::LocalDayMapper,
    ) -> crate::error::Result<Vec<f64>> {
        let starts = mapper.day_starts_back(now, 7);
        let mut out = Vec::with_capacity(7);
        for pair in starts.windows(2) {
            out.push(self.applied_effective_tokens_between(
                pair[0].to_offset(time::UtcOffset::UTC),
                pair[1].to_offset(time::UtcOffset::UTC),
            )?);
        }
        Ok(out)
    }

    pub fn best_day_effective_tokens(&self) -> crate::error::Result<f64> {
        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(MAX(daily_total), 0.0) FROM (
                SELECT period_date, SUM(effective_tokens) AS daily_total
                FROM (
                    SELECT period_date, effective_tokens FROM usage_events
                    WHERE feedable = 1
                    UNION ALL
                    SELECT period_date, effective_tokens FROM daily_aggregates
                )
                GROUP BY period_date
            )",
        )?;
        let best: f64 = stmt.query_row([], |row| row.get(0))?;
        Ok(best)
    }

    fn migrate(&self) -> crate::error::Result<()> {
        self.conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS provider_cursors (
                provider_surface TEXT NOT NULL,
                cursor_key TEXT NOT NULL,
                cursor_value TEXT NOT NULL,
                provider_version TEXT NOT NULL,
                parser_version TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (provider_surface, cursor_key)
            );

            CREATE TABLE IF NOT EXISTS token_contract_state (
                token_contract TEXT PRIMARY KEY,
                activated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS usage_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider_surface TEXT NOT NULL,
                provider_version TEXT NOT NULL,
                parser_version TEXT NOT NULL,
                command TEXT NOT NULL,
                source_surface TEXT NOT NULL,
                period_start TEXT NOT NULL,
                observed_at TEXT NOT NULL,
                bucket_at TEXT NOT NULL,
                period_date TEXT NOT NULL,
                model TEXT,
                input_tokens REAL NOT NULL,
                output_tokens REAL NOT NULL,
                cache_creation_tokens REAL NOT NULL,
                cache_read_tokens REAL NOT NULL,
                reasoning_output_tokens REAL NOT NULL,
                effective_tokens REAL NOT NULL,
                total_tokens REAL NOT NULL DEFAULT 0.0,
                token_contract TEXT NOT NULL DEFAULT 'weighted_effective_v1',
                cost_usd REAL,
                confidence TEXT NOT NULL,
                provider_delta_id TEXT,
                bucket_index INTEGER NOT NULL DEFAULT 0,
                bucket_count INTEGER NOT NULL DEFAULT 1,
                applied_at TEXT,
                feedable INTEGER NOT NULL DEFAULT 1,
                provider_cursor_key TEXT,
                provider_cursor_value TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_usage_events_period_start
                ON usage_events(period_start);

            CREATE TABLE IF NOT EXISTS daily_aggregates (
                provider_surface TEXT NOT NULL,
                period_date TEXT NOT NULL,
                source_surface TEXT NOT NULL,
                input_tokens REAL NOT NULL DEFAULT 0.0,
                output_tokens REAL NOT NULL DEFAULT 0.0,
                cache_creation_tokens REAL NOT NULL DEFAULT 0.0,
                cache_read_tokens REAL NOT NULL DEFAULT 0.0,
                reasoning_output_tokens REAL NOT NULL DEFAULT 0.0,
                effective_tokens REAL NOT NULL DEFAULT 0.0,
                cost_usd REAL NOT NULL DEFAULT 0.0,
                event_count INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (provider_surface, period_date, source_surface)
            );

            CREATE TABLE IF NOT EXISTS provider_diagnostics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider_surface TEXT NOT NULL,
                code TEXT NOT NULL,
                message TEXT NOT NULL,
                recorded_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS provider_snapshot_batches (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                collector_scope_id TEXT NOT NULL,
                collector_surface TEXT NOT NULL,
                command TEXT NOT NULL,
                token_contract TEXT NOT NULL,
                requested_provider_days_json TEXT NOT NULL,
                provider_version TEXT NOT NULL,
                parser_version TEXT NOT NULL,
                observed_at TEXT NOT NULL,
                completion_status TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS provider_snapshot_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                batch_id INTEGER,
                replacement_scope_id TEXT NOT NULL,
                collector_scope_id TEXT NOT NULL,
                collector_surface TEXT NOT NULL,
                command TEXT NOT NULL,
                token_contract TEXT NOT NULL,
                provider_day TEXT NOT NULL,
                provider_version TEXT NOT NULL,
                parser_version TEXT NOT NULL,
                observed_at TEXT NOT NULL,
                completion_status TEXT NOT NULL,
                reason_code TEXT,
                FOREIGN KEY(batch_id) REFERENCES provider_snapshot_batches(id)
            );

            CREATE TABLE IF NOT EXISTS provider_snapshot_rows (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id INTEGER NOT NULL,
                replacement_scope_id TEXT NOT NULL,
                collector_scope_id TEXT NOT NULL,
                collector_surface TEXT NOT NULL,
                command TEXT NOT NULL,
                token_contract TEXT NOT NULL,
                accounting_source TEXT NOT NULL,
                provider_day TEXT NOT NULL,
                model TEXT,
                source_surface TEXT NOT NULL,
                provider_period TEXT NOT NULL,
                raw_source_id_hash TEXT,
                cursor_key_hash TEXT NOT NULL,
                input_tokens REAL,
                output_tokens REAL,
                cache_creation_tokens REAL,
                cache_read_tokens REAL,
                reasoning_output_tokens REAL,
                total_tokens REAL NOT NULL,
                cost_usd REAL,
                confidence TEXT NOT NULL,
                status TEXT NOT NULL,
                first_observed_at TEXT NOT NULL,
                last_observed_at TEXT NOT NULL,
                FOREIGN KEY(run_id) REFERENCES provider_snapshot_runs(id)
            );

            CREATE TABLE IF NOT EXISTS provider_corrections (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                correction_kind TEXT NOT NULL,
                token_contract TEXT NOT NULL,
                accounting_source TEXT NOT NULL,
                provider_day TEXT NOT NULL,
                model TEXT,
                previous_total_tokens REAL NOT NULL,
                current_total_tokens REAL NOT NULL,
                decrease_tokens REAL NOT NULL,
                previous_raw_buckets_json TEXT,
                current_raw_buckets_json TEXT,
                collector_surface TEXT NOT NULL,
                cursor_key_hash TEXT,
                batch_id INTEGER,
                run_id INTEGER,
                recorded_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS provider_snapshot_diagnostics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                diagnostic_kind TEXT NOT NULL,
                collector_scope_id TEXT NOT NULL,
                replacement_scope_id TEXT,
                requested_provider_days_json TEXT NOT NULL,
                provider_day TEXT,
                reason_code TEXT NOT NULL,
                message TEXT NOT NULL,
                batch_id INTEGER,
                run_id INTEGER,
                recorded_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS provider_canonical_collectors (
                token_contract TEXT NOT NULL,
                accounting_source TEXT NOT NULL,
                provider_day TEXT NOT NULL,
                collector_scope_id TEXT NOT NULL,
                replacement_scope_id TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (token_contract, accounting_source, provider_day)
            );

            CREATE TABLE IF NOT EXISTS provider_source_contacts (
                token_contract TEXT NOT NULL,
                accounting_source TEXT NOT NULL,
                contact_kind TEXT NOT NULL,
                recorded_at TEXT NOT NULL,
                PRIMARY KEY (token_contract, accounting_source)
            );

            CREATE TABLE IF NOT EXISTS provider_feed_highwaters (
                highwater_kind TEXT NOT NULL,
                token_contract TEXT NOT NULL,
                accounting_source TEXT NOT NULL,
                provider_day TEXT,
                provider_day_key TEXT NOT NULL DEFAULT '',
                model TEXT,
                model_key TEXT NOT NULL DEFAULT '',
                provider_surface TEXT,
                provider_surface_key TEXT NOT NULL DEFAULT '',
                cursor_key_hash TEXT,
                cursor_key_hash_key TEXT NOT NULL DEFAULT '',
                total_high_water REAL NOT NULL,
                latest_raw_buckets_json TEXT,
                exact_raw_buckets_json TEXT,
                bucket_confidence TEXT NOT NULL,
                unshaped_total_only_tokens REAL NOT NULL DEFAULT 0.0,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (
                    highwater_kind,
                    token_contract,
                    accounting_source,
                    provider_day_key,
                    model_key,
                    provider_surface_key,
                    cursor_key_hash_key
                )
            );

            CREATE TABLE IF NOT EXISTS lifetime_counters (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                effective_tokens REAL NOT NULL DEFAULT 0.0,
                event_count INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL
            );
            ",
        )?;
        ensure_usage_event_column(
            &self.conn,
            "observed_at",
            "ALTER TABLE usage_events ADD COLUMN observed_at TEXT;",
            "UPDATE usage_events SET observed_at = period_start WHERE observed_at IS NULL;",
        )?;
        ensure_usage_event_column(
            &self.conn,
            "bucket_at",
            "ALTER TABLE usage_events ADD COLUMN bucket_at TEXT;",
            "UPDATE usage_events SET bucket_at = period_start WHERE bucket_at IS NULL;",
        )?;
        ensure_usage_event_column(
            &self.conn,
            "provider_delta_id",
            "ALTER TABLE usage_events ADD COLUMN provider_delta_id TEXT;",
            "",
        )?;
        ensure_usage_event_column(
            &self.conn,
            "bucket_index",
            "ALTER TABLE usage_events ADD COLUMN bucket_index INTEGER NOT NULL DEFAULT 0;",
            "",
        )?;
        ensure_usage_event_column(
            &self.conn,
            "bucket_count",
            "ALTER TABLE usage_events ADD COLUMN bucket_count INTEGER NOT NULL DEFAULT 1;",
            "",
        )?;
        ensure_usage_event_column(
            &self.conn,
            "applied_at",
            "ALTER TABLE usage_events ADD COLUMN applied_at TEXT;",
            "UPDATE usage_events SET applied_at = observed_at WHERE applied_at IS NULL;",
        )?;
        ensure_usage_event_column(
            &self.conn,
            "feedable",
            "ALTER TABLE usage_events ADD COLUMN feedable INTEGER NOT NULL DEFAULT 1;",
            "",
        )?;
        ensure_usage_event_column(
            &self.conn,
            "provider_cursor_key",
            "ALTER TABLE usage_events ADD COLUMN provider_cursor_key TEXT;",
            "",
        )?;
        ensure_usage_event_column(
            &self.conn,
            "provider_cursor_value",
            "ALTER TABLE usage_events ADD COLUMN provider_cursor_value TEXT;",
            "",
        )?;
        ensure_usage_event_column(
            &self.conn,
            "total_tokens",
            "ALTER TABLE usage_events ADD COLUMN total_tokens REAL NOT NULL DEFAULT 0.0;",
            "UPDATE usage_events SET total_tokens = effective_tokens WHERE total_tokens = 0.0;",
        )?;
        ensure_usage_event_column(
            &self.conn,
            "token_contract",
            "ALTER TABLE usage_events ADD COLUMN token_contract TEXT NOT NULL DEFAULT 'weighted_effective_v1';",
            "",
        )?;
        backfill_legacy_seeded_feedable_rows(&self.conn)?;
        mark_unified_helper_rows_non_feedable(&self.conn)?;
        self.conn.execute_batch(
            "
            CREATE INDEX IF NOT EXISTS idx_usage_events_observed_at
                ON usage_events(observed_at);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_usage_events_provider_delta_bucket
                ON usage_events(provider_delta_id, bucket_index)
                WHERE provider_delta_id IS NOT NULL;
            CREATE INDEX IF NOT EXISTS idx_usage_events_applied_at
                ON usage_events(applied_at);
            CREATE INDEX IF NOT EXISTS idx_usage_events_bucket_at
                ON usage_events(bucket_at);
            CREATE INDEX IF NOT EXISTS idx_provider_snapshot_rows_visible
                ON provider_snapshot_rows(token_contract, accounting_source, provider_day, status);
            CREATE INDEX IF NOT EXISTS idx_provider_snapshot_runs_scope
                ON provider_snapshot_runs(replacement_scope_id, token_contract, provider_day, observed_at);
            CREATE INDEX IF NOT EXISTS idx_provider_snapshot_diagnostics_scope
                ON provider_snapshot_diagnostics(collector_scope_id, provider_day, recorded_at);
            CREATE INDEX IF NOT EXISTS idx_provider_corrections_day
                ON provider_corrections(token_contract, accounting_source, provider_day, recorded_at);
            ",
        )?;
        migrate_provider_cursors_to_source_label(&self.conn)?;
        Ok(())
    }
}

fn migrate_provider_cursors_to_source_label(conn: &Connection) -> rusqlite::Result<()> {
    #[derive(serde::Deserialize)]
    struct SourceKey {
        provider_surface: String,
    }

    let mut stmt = conn.prepare(
        "SELECT provider_surface, cursor_key FROM provider_cursors
         WHERE cursor_key LIKE '{%'",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    for (old_surface, key_json) in rows {
        if let Ok(key) = serde_json::from_str::<SourceKey>(&key_json) {
            conn.execute(
                "UPDATE OR IGNORE provider_cursors
                 SET provider_surface = ?1
                 WHERE provider_surface = ?2 AND cursor_key = ?3",
                params![key.provider_surface, old_surface, key_json],
            )?;
            if conn.changes() == 0 && old_surface != key.provider_surface {
                // The update was ignored because a source-label row with the
                // same cursor key already exists. Drop the stale helper-surface
                // row so the partition does not leak back to the helper.
                conn.execute(
                    "DELETE FROM provider_cursors
                     WHERE provider_surface = ?1 AND cursor_key = ?2",
                    params![old_surface, key_json],
                )?;
            }
        }
    }
    Ok(())
}

fn backfill_legacy_seeded_feedable_rows(conn: &Connection) -> crate::error::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT provider_surface, recorded_at, message
         FROM provider_diagnostics
         WHERE code = ?1
         ORDER BY id ASC",
    )?;
    let diagnostics = stmt
        .query_map(
            params![crate::game::runtime::SOURCE_FIRST_CONTACT_CODE],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);

    for (provider_surface, recorded_at, message) in diagnostics {
        let Some(expected_count) = parse_source_first_contact_seed_count(&message) else {
            continue;
        };
        let mut candidate_stmt = conn.prepare(
            "SELECT id
             FROM usage_events
             WHERE provider_surface = ?1
               AND source_surface = 'daily'
               AND applied_at = ?2
               AND provider_delta_id IS NOT NULL
               AND provider_cursor_key IS NOT NULL
               AND provider_cursor_value IS NOT NULL
               AND bucket_index = 0
               AND bucket_count = 1
               AND feedable = 1
             ORDER BY id ASC",
        )?;
        let candidate_ids = candidate_stmt
            .query_map(params![provider_surface, recorded_at], |row| {
                row.get::<_, i64>(0)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(candidate_stmt);

        // Be conservative: if the diagnostic's seeded-row count does not
        // match the exact candidate set, leave the rows untouched rather than
        // risk reclassifying ordinary one-bucket applied history.
        if candidate_ids.len() != expected_count || candidate_ids.is_empty() {
            continue;
        }

        let placeholders = candidate_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        conn.execute(
            &format!("UPDATE usage_events SET feedable = 0 WHERE id IN ({placeholders})"),
            rusqlite::params_from_iter(candidate_ids.iter()),
        )?;
    }

    Ok(())
}

fn mark_unified_helper_rows_non_feedable(conn: &Connection) -> crate::error::Result<()> {
    conn.execute(
        "UPDATE usage_events
         SET feedable = 0
         WHERE provider_surface = 'unified'
           AND feedable = 1",
        [],
    )?;
    Ok(())
}

fn parse_source_first_contact_seed_count(message: &str) -> Option<usize> {
    message
        .rsplit_once(": ")
        .and_then(|(_, tail)| tail.strip_suffix(" historical rows seeded without feeding"))
        .and_then(|count| count.parse().ok())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> crate::error::Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for name in rows {
        if name? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn ensure_usage_event_column(
    conn: &Connection,
    name: &str,
    definition: &str,
    backfill_sql: &str,
) -> crate::error::Result<()> {
    if !column_exists(conn, "usage_events", name)? {
        conn.execute_batch(definition)?;
        if !backfill_sql.is_empty() {
            conn.execute_batch(backfill_sql)?;
        }
    }
    Ok(())
}

fn provider_days_json(days: &[Date]) -> crate::error::Result<String> {
    Ok(serde_json::to_string(
        &days.iter().map(Date::to_string).collect::<Vec<_>>(),
    )?)
}

fn canonical_visible_source_day_snapshots(
    tx: &rusqlite::Transaction<'_>,
    token_contract: &str,
    day: Date,
) -> crate::error::Result<BTreeMap<String, SourceDaySnapshot>> {
    let mut stmt = tx.prepare(
        "SELECT r.accounting_source, r.total_tokens, r.model, r.source_surface,
                r.provider_period, r.raw_source_id_hash, r.cursor_key_hash
         FROM provider_snapshot_rows AS r
         JOIN provider_canonical_collectors AS c
           ON c.token_contract = r.token_contract
          AND c.accounting_source = r.accounting_source
          AND c.provider_day = r.provider_day
          AND c.replacement_scope_id = r.replacement_scope_id
         WHERE r.token_contract = ?1
           AND r.provider_day = ?2
           AND r.status = 'active'",
    )?;
    let mut snapshots = BTreeMap::new();
    let rows = stmt.query_map(params![token_contract, day.to_string()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, String>(6)?,
        ))
    })?;
    for row in rows {
        let (
            accounting_source,
            total_tokens,
            model,
            source_surface,
            provider_period,
            raw_source_id_hash,
            cursor_key_hash,
        ) = row?;
        let entry = snapshots
            .entry(accounting_source)
            .or_insert_with(|| SourceDaySnapshot {
                total_tokens: 0.0,
                identity_fingerprints: BTreeSet::new(),
            });
        entry.total_tokens += total_tokens;
        entry.identity_fingerprints.insert(snapshot_row_fingerprint(
            model.as_deref(),
            &source_surface,
            &provider_period,
            raw_source_id_hash.as_deref(),
            &cursor_key_hash,
        ));
    }
    Ok(snapshots)
}

fn snapshot_row_fingerprint(
    model: Option<&str>,
    source_surface: &str,
    provider_period: &str,
    raw_source_id_hash: Option<&str>,
    cursor_key_hash: &str,
) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        model.unwrap_or(""),
        source_surface,
        provider_period,
        raw_source_id_hash.unwrap_or(""),
        cursor_key_hash
    )
}

fn supersede_previous_snapshot_rows(
    tx: &rusqlite::Transaction<'_>,
    replacement_scope_id: &str,
    token_contract: &str,
    day: Date,
) -> crate::error::Result<()> {
    tx.execute(
        "UPDATE provider_snapshot_rows
         SET status = 'superseded'
         WHERE replacement_scope_id = ?1
           AND token_contract = ?2
           AND provider_day = ?3
           AND status = 'active'",
        params![replacement_scope_id, token_contract, day.to_string()],
    )?;
    Ok(())
}

fn supersede_previous_snapshot_rows_for_day(
    tx: &rusqlite::Transaction<'_>,
    token_contract: &str,
    day: Date,
) -> crate::error::Result<()> {
    tx.execute(
        "UPDATE provider_snapshot_rows
         SET status = 'superseded'
         WHERE token_contract = ?1
           AND provider_day = ?2
           AND status = 'active'",
        params![token_contract, day.to_string()],
    )?;
    Ok(())
}

fn insert_snapshot_rows(
    tx: &rusqlite::Transaction<'_>,
    run_id: i64,
    rows: &[&crate::usage::snapshot::ProviderSnapshotRowInput],
    observed_at: OffsetDateTime,
) -> crate::error::Result<()> {
    let observed_at = format_time(observed_at)?;
    for row in rows {
        let raw = row.raw_token_buckets;
        tx.execute(
            "INSERT INTO provider_snapshot_rows (
                run_id, replacement_scope_id, collector_scope_id, collector_surface,
                command, token_contract, accounting_source, provider_day, model,
                source_surface, provider_period, raw_source_id_hash, cursor_key_hash,
                input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens,
                reasoning_output_tokens, total_tokens, cost_usd, confidence, status,
                first_observed_at, last_observed_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, 'active', ?22, ?23
            )",
            params![
                run_id,
                row.replacement_scope_id,
                row.collector_scope_id,
                row.collector_surface,
                row.command,
                row.token_contract,
                row.accounting_source,
                row.provider_day.to_string(),
                row.model,
                row.source_surface,
                row.provider_period,
                row.raw_source_id_hash,
                row.cursor_key_hash,
                raw.map(|totals| totals.uncached_input as f64),
                raw.map(|totals| totals.output as f64),
                raw.map(|totals| totals.cache_creation as f64),
                raw.map(|totals| totals.cache_read as f64),
                raw.map(|totals| totals.reasoning_output as f64),
                row.total_tokens,
                row.cost_usd,
                row.confidence,
                observed_at,
                observed_at,
            ],
        )?;
        upsert_provider_cursor(tx, &row.cursor_update, &observed_at)?;
    }
    Ok(())
}

fn refresh_canonical_collectors(
    tx: &rusqlite::Transaction<'_>,
    replacement_scope_id: &str,
    token_contract: &str,
    day: Date,
    rows: &[&crate::usage::snapshot::ProviderSnapshotRowInput],
    observed_at: OffsetDateTime,
) -> crate::error::Result<()> {
    let observed_at = format_time(observed_at)?;
    let sources = rows
        .iter()
        .map(|row| row.accounting_source.as_str())
        .collect::<BTreeSet<_>>();
    for source in sources {
        tx.execute(
            "INSERT INTO provider_canonical_collectors (
                token_contract, accounting_source, provider_day, collector_scope_id,
                replacement_scope_id, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(token_contract, accounting_source, provider_day) DO UPDATE SET
                collector_scope_id = excluded.collector_scope_id,
                replacement_scope_id = excluded.replacement_scope_id,
                updated_at = excluded.updated_at",
            params![
                token_contract,
                source,
                day.to_string(),
                rows.iter()
                    .find(|row| row.accounting_source == source)
                    .map(|row| row.collector_scope_id.as_str())
                    .unwrap_or(replacement_scope_id),
                replacement_scope_id,
                observed_at,
            ],
        )?;
    }
    Ok(())
}

fn record_snapshot_corrections(
    tx: &rusqlite::Transaction<'_>,
    batch_id: i64,
    run_id: i64,
    batch: &crate::usage::snapshot::ProviderSnapshotBatchInput,
    day: Date,
    rows: &[&crate::usage::snapshot::ProviderSnapshotRowInput],
    comparison: SourceDaySnapshotComparison<'_>,
) -> crate::error::Result<()> {
    let requested_days_json = provider_days_json(&[day])?;
    let recorded_at = format_time(batch.observed_at)?;
    for (source, previous_snapshot) in comparison.previous {
        let current_snapshot = comparison.current.get(source);
        let current_total = current_snapshot
            .map(|snapshot| snapshot.total_tokens)
            .unwrap_or(0.0);
        if previous_snapshot.total_tokens > current_total {
            let cursor_key_hash = rows
                .iter()
                .find(|row| row.accounting_source == *source)
                .map(|row| row.cursor_key_hash.as_str());
            tx.execute(
                "INSERT INTO provider_corrections (
                    correction_kind, token_contract, accounting_source, provider_day, model,
                    previous_total_tokens, current_total_tokens, decrease_tokens,
                    previous_raw_buckets_json, current_raw_buckets_json, collector_surface,
                    cursor_key_hash, batch_id, run_id, recorded_at
                ) VALUES (
                    'source_day_decrease', ?1, ?2, ?3, NULL, ?4, ?5, ?6,
                    NULL, NULL, ?7, ?8, ?9, ?10, ?11
                )",
                params![
                    batch.token_contract,
                    source,
                    day.to_string(),
                    previous_snapshot.total_tokens,
                    current_total,
                    previous_snapshot.total_tokens - current_total,
                    batch.collector_surface,
                    cursor_key_hash,
                    batch_id,
                    run_id,
                    recorded_at,
                ],
            )?;
        } else if (previous_snapshot.total_tokens - current_total).abs() < f64::EPSILON
            && current_snapshot
                .map(|snapshot| {
                    snapshot.identity_fingerprints != previous_snapshot.identity_fingerprints
                })
                .unwrap_or(false)
        {
            tx.execute(
                "INSERT INTO provider_snapshot_diagnostics (
                    diagnostic_kind, collector_scope_id, replacement_scope_id,
                    requested_provider_days_json, provider_day, reason_code, message,
                    batch_id, run_id, recorded_at
                ) VALUES (
                    'identity_remap', ?1, ?2, ?3, ?4, 'identity_remap',
                    'snapshot row identity changed without source-day total change',
                    ?5, ?6, ?7
                )",
                params![
                    batch.collector_scope_id,
                    rows.iter()
                        .find(|row| row.accounting_source == *source)
                        .map(|row| row.replacement_scope_id.as_str())
                        .unwrap_or(batch.collector_scope_id.as_str()),
                    requested_days_json,
                    day.to_string(),
                    batch_id,
                    run_id,
                    recorded_at,
                ],
            )?;
        }
    }
    Ok(())
}

fn insert_snapshot_diagnostics(
    tx: &rusqlite::Transaction<'_>,
    batch_id: i64,
    run_id: Option<i64>,
    diagnostics: &[&crate::usage::snapshot::ProviderSnapshotDiagnosticInput],
) -> crate::error::Result<()> {
    for diagnostic in diagnostics {
        tx.execute(
            "INSERT INTO provider_snapshot_diagnostics (
                diagnostic_kind, collector_scope_id, replacement_scope_id,
                requested_provider_days_json, provider_day, reason_code, message,
                batch_id, run_id, recorded_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                diagnostic.diagnostic_kind,
                diagnostic.collector_scope_id,
                diagnostic.replacement_scope_id,
                provider_days_json(&diagnostic.requested_provider_days)?,
                diagnostic.provider_day.map(|day| day.to_string()),
                diagnostic.reason_code,
                diagnostic.message,
                batch_id,
                run_id,
                format_time(diagnostic.observed_at)?,
            ],
        )?;
    }
    Ok(())
}

fn add_lifetime_counter(
    tx: &rusqlite::Transaction<'_>,
    effective_tokens: f64,
) -> rusqlite::Result<()> {
    let now = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
    tx.execute(
        "INSERT INTO lifetime_counters (id, effective_tokens, event_count, updated_at)
         VALUES (1, ?1, 1, ?2)
         ON CONFLICT(id) DO UPDATE SET
            effective_tokens = lifetime_counters.effective_tokens + excluded.effective_tokens,
            event_count = lifetime_counters.event_count + 1,
            updated_at = excluded.updated_at",
        params![effective_tokens, now],
    )?;
    Ok(())
}

fn format_time(value: OffsetDateTime) -> crate::error::Result<String> {
    value
        .format(&Rfc3339)
        .map_err(|err| crate::error::GlorpError::Message(format!("invalid timestamp: {err}")))
}

fn parse_time_for_sql(value: &str) -> rusqlite::Result<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))
}

/// Upsert one provider cursor inside an existing transaction. Shared by
/// `advance_cursors`, `mark_events_applied_and_advance_cursors`, and
/// `refuse_poll_discontinuity` so the conflict-update column set cannot
/// drift between the three cursor-advance paths.
fn upsert_provider_cursor(
    tx: &rusqlite::Transaction<'_>,
    update: &ProviderCursorUpdate,
    updated_at: &str,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO provider_cursors (
            provider_surface,
            cursor_key,
            cursor_value,
            provider_version,
            parser_version,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(provider_surface, cursor_key) DO UPDATE SET
            cursor_value = excluded.cursor_value,
            provider_version = excluded.provider_version,
            parser_version = excluded.parser_version,
            updated_at = excluded.updated_at",
        params![
            update.provider_surface,
            update.cursor_key,
            update.cursor_value,
            update.provider_version,
            update.parser_version,
            updated_at,
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::day_axis::LocalDayMapper;
    use rusqlite::Connection;
    use tempfile::tempdir;
    use time::macros::datetime;

    fn sample_event_at(observed_at: OffsetDateTime, tokens: f64) -> NormalizedUsageEvent {
        NormalizedUsageEvent {
            observed_at,
            bucket_at: observed_at,
            ..NormalizedUsageEvent::for_test_at(observed_at, tokens)
        }
    }

    #[test]
    fn token_totals_by_source_between_groups_and_sums_unbounded() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let start = now - time::Duration::hours(2);
        let end = now + time::Duration::seconds(1);
        let inside_claude_a = NormalizedUsageEvent {
            provider_surface: "claude-code".to_string(),
            ..sample_event_at(now - time::Duration::minutes(5), 100.0)
        };
        let inside_claude_b = NormalizedUsageEvent {
            provider_surface: "claude-code".to_string(),
            ..sample_event_at(now - time::Duration::minutes(30), 250.0)
        };
        let inside_codex = NormalizedUsageEvent {
            provider_surface: "codex".to_string(),
            ..sample_event_at(now - time::Duration::minutes(10), 400.0)
        };
        let before_window = NormalizedUsageEvent {
            provider_surface: "claude-code".to_string(),
            ..sample_event_at(now - time::Duration::hours(3), 9_999.0)
        };
        for e in [
            &inside_claude_a,
            &inside_claude_b,
            &inside_codex,
            &before_window,
        ] {
            store.insert_event(e).unwrap();
        }
        // Stress the unbounded contract: 600 tiny rows from one source must
        // all be summed, not clipped to a 500-row recent-events limit.
        for i in 0..600 {
            let e = NormalizedUsageEvent {
                provider_surface: "codex".to_string(),
                ..sample_event_at(now - time::Duration::seconds(i), 1.0)
            };
            store.insert_event(&e).unwrap();
        }

        let totals = store.token_totals_by_source_between(start, end).unwrap();
        let map: std::collections::BTreeMap<String, f64> = totals.into_iter().collect();
        assert_eq!(map.get("claude-code").copied().unwrap(), 350.0);
        assert_eq!(map.get("codex").copied().unwrap(), 400.0 + 600.0);
    }

    #[test]
    fn token_totals_by_source_between_includes_both_boundaries() {
        // Inclusive on both sides — RFC3339 lexical sort (`Z` > `.`) makes
        // half-open `< end` filters drop events whose timestamp lacks
        // fractional seconds, which silently undercounts.
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let one_hour_ago = now - time::Duration::hours(1);
        store.insert_event(&sample_event_at(now, 5_555.0)).unwrap();
        store
            .insert_event(&sample_event_at(one_hour_ago, 1_111.0))
            .unwrap();
        let totals = store
            .token_totals_by_source_between(one_hour_ago, now)
            .unwrap();
        let sum: f64 = totals.iter().map(|(_, v)| v).sum();
        assert_eq!(sum, 5_555.0 + 1_111.0);
    }

    #[test]
    fn token_totals_by_source_between_excludes_outside_window() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store
            .insert_event(&sample_event_at(now - time::Duration::hours(2), 9_999.0))
            .unwrap();
        let totals = store
            .token_totals_by_source_between(now - time::Duration::hours(1), now)
            .unwrap();
        assert!(totals.is_empty(), "outside-window event must be excluded");
    }

    #[test]
    fn migration_marks_historical_unified_helper_rows_non_feedable() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("usage.sqlite");
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        {
            let mut store = UsageStore::open(&path).unwrap();
            store
                .insert_event(&NormalizedUsageEvent {
                    provider_surface: "unified".to_string(),
                    ..sample_event_at(now, 1_000.0)
                })
                .unwrap();
            store
                .insert_event(&NormalizedUsageEvent {
                    provider_surface: "claude-code".to_string(),
                    ..sample_event_at(now, 2_000.0)
                })
                .unwrap();
        }

        let store = UsageStore::open(&path).unwrap();
        let totals = store
            .canonical_total_tokens_by_source_between(
                now - time::Duration::seconds(1),
                now + time::Duration::seconds(1),
            )
            .unwrap();
        let map: std::collections::BTreeMap<String, f64> = totals.into_iter().collect();

        assert_eq!(map.get("claude-code"), Some(&2_000.0));
        assert!(!map.contains_key("unified"));
        let unified_feedable: i64 = store
            .conn
            .query_row(
                "SELECT feedable FROM usage_events WHERE provider_surface = 'unified'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unified_feedable, 0);
    }

    #[test]
    fn events_within_returns_events_inside_window_only() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let inside_a = sample_event_at(now - time::Duration::minutes(10), 1_000.0);
        let inside_b = sample_event_at(now - time::Duration::hours(1), 2_000.0);
        let outside = sample_event_at(now - time::Duration::hours(3), 9_999.0);
        for e in [&inside_a, &inside_b, &outside] {
            store.insert_event(e).unwrap();
        }
        let got = store.events_within(time::Duration::hours(2), now).unwrap();
        let totals: Vec<f64> = got.iter().map(|e| e.effective_tokens).collect();
        assert!(totals.contains(&1_000.0), "inside_a must be present");
        assert!(totals.contains(&2_000.0), "inside_b must be present");
        assert!(!totals.contains(&9_999.0), "outside must be excluded");
    }

    #[test]
    fn events_within_boundary_is_inclusive_at_lower_bound() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let on_boundary = sample_event_at(now - time::Duration::hours(2), 5_555.0);
        store.insert_event(&on_boundary).unwrap();
        let got = store.events_within(time::Duration::hours(2), now).unwrap();
        assert_eq!(
            got.len(),
            1,
            "boundary event must be included (>= comparison)"
        );
    }

    #[test]
    fn seven_day_token_history_returns_seven_oldest_first() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let today = OffsetDateTime::now_utc();
        let day0 = today - time::Duration::days(6);
        let day6 = today;
        store
            .insert_event(&NormalizedUsageEvent::for_test_at(day0, 1000.0))
            .unwrap();
        store
            .insert_event(&NormalizedUsageEvent::for_test_at(day6, 5000.0))
            .unwrap();

        let history = store
            .seven_day_token_history(today, LocalDayMapper::Fixed(time::UtcOffset::UTC))
            .unwrap();
        assert_eq!(history.len(), 7);
        assert_eq!(history[0], 1000.0);
        assert_eq!(history[6], 5000.0);
        for v in &history[1..6] {
            assert_eq!(*v, 0.0);
        }
    }

    #[test]
    fn seven_day_token_history_sums_multiple_events_per_day() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let today = OffsetDateTime::now_utc();
        store
            .insert_event(&NormalizedUsageEvent::for_test_at(today, 1500.0))
            .unwrap();
        store
            .insert_event(&NormalizedUsageEvent::for_test_at(today, 2500.0))
            .unwrap();

        let history = store
            .seven_day_token_history(today, LocalDayMapper::Fixed(time::UtcOffset::UTC))
            .unwrap();
        assert_eq!(history[6], 4000.0);
    }

    #[test]
    fn seven_day_token_history_zero_for_empty_store() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let today = OffsetDateTime::now_utc();
        let history = store
            .seven_day_token_history(today, LocalDayMapper::Fixed(time::UtcOffset::UTC))
            .unwrap();
        assert_eq!(history, vec![0.0; 7]);
    }

    #[test]
    fn today_effective_tokens_groups_on_local_bucket_at_day_applied_only() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        // 2026-06-09 01:00 UTC == 2026-06-08 17:00 local at UTC-8: yesterday locally.
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::from_hms(-8, 0, 0).unwrap());
        let now = datetime!(2026-06-09 18:00 UTC); // 10:00 local June 9
        store
            .insert_event(&sample_event_at(datetime!(2026-06-09 01:00 UTC), 7_777.0))
            .unwrap(); // local June 8 — must NOT count
        store
            .insert_event(&sample_event_at(datetime!(2026-06-09 17:00 UTC), 1_111.0))
            .unwrap(); // local June 9 09:00 — counts
        assert_eq!(store.today_effective_tokens(now, mapper).unwrap(), 1_111.0);
    }

    #[test]
    fn seven_day_token_history_uses_local_bucket_at_days_and_no_aggregates_union() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::UTC);
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store.insert_event(&sample_event_at(now, 1_234.0)).unwrap();
        store
            .insert_event(&sample_event_at(now - time::Duration::days(6), 7_777.0))
            .unwrap();
        // A daily_aggregates row must NOT surface: compaction cutoff is 90
        // days, so aggregates cannot occur inside a 7-day window. This
        // supersedes seven_day_token_history_includes_compacted_days.
        store
            .conn
            .execute(
                "INSERT INTO daily_aggregates (
                    provider_surface, period_date, source_surface,
                    input_tokens, output_tokens, cache_creation_tokens,
                    cache_read_tokens, reasoning_output_tokens,
                    effective_tokens, cost_usd, event_count
                ) VALUES ('claude-code', ?1, 'daily', 0, 0, 0, 0, 0, 5555.0, 0, 1)",
                rusqlite::params![(now.date() - time::Duration::days(3)).to_string()],
            )
            .unwrap();
        let history = store.seven_day_token_history(now, mapper).unwrap();
        assert_eq!(history.len(), 7);
        assert_eq!(history[0], 7_777.0);
        assert_eq!(
            history[3], 0.0,
            "aggregates must not leak into the 7-day window"
        );
        assert_eq!(history[6], 1_234.0);
    }

    #[test]
    fn best_day_returns_largest_daily_total_from_events_only() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store.insert_event(&sample_event_at(now, 3_000.0)).unwrap();
        store.insert_event(&sample_event_at(now, 2_000.0)).unwrap();
        store
            .insert_event(&sample_event_at(now - time::Duration::days(1), 4_000.0))
            .unwrap();
        let best = store.best_day_effective_tokens().unwrap();
        assert_eq!(best, 5_000.0, "today sums to 5k, beats yesterday's 4k");
    }

    #[test]
    fn best_day_returns_zero_when_empty() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        assert_eq!(store.best_day_effective_tokens().unwrap(), 0.0);
    }

    #[test]
    fn best_day_sums_overlap_between_events_and_aggregates() {
        // Compaction window: same period_date appears in both tables.
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let period_date = now.date().to_string();
        store.insert_event(&sample_event_at(now, 1_000.0)).unwrap();
        store
            .conn
            .execute(
                "INSERT INTO daily_aggregates (
                    provider_surface, period_date, source_surface,
                    input_tokens, output_tokens, cache_creation_tokens,
                    cache_read_tokens, reasoning_output_tokens,
                    effective_tokens, cost_usd, event_count
                ) VALUES ('claude-code', ?1, 'daily', 0, 0, 0, 0, 0, 2000.0, 0, 1)",
                rusqlite::params![period_date],
            )
            .unwrap();
        let best = store.best_day_effective_tokens().unwrap();
        assert_eq!(best, 3_000.0, "events 1k + aggregate 2k = 3k");
    }

    #[test]
    fn compact_before_keeps_rows_with_recent_bucket_at_even_when_period_start_is_ancient() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let ancient_period = now - time::Duration::days(120);
        // Long-gap resume shape: provider day is 120 days old, but the smear
        // anchored the bucket at poll time (now).
        let event = NormalizedUsageEvent {
            period_start: ancient_period,
            observed_at: now,
            bucket_at: now,
            ..NormalizedUsageEvent::for_test_at(now, 9_999.0)
        };
        store.insert_event(&event).unwrap(); // insert_event rows are born applied
        store
            .compact_before(now - time::Duration::days(90))
            .unwrap();
        let totals = store
            .token_totals_by_source_between(now - time::Duration::minutes(10), now)
            .unwrap();
        let sum: f64 = totals.iter().map(|(_, v)| v).sum();
        assert_eq!(
            sum, 9_999.0,
            "rows still inside live bucket_at windows must survive compaction"
        );
    }

    #[test]
    fn compact_before_still_compacts_rows_old_on_both_axes() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let old = now - time::Duration::days(120);
        store.insert_event(&sample_event_at(old, 4_444.0)).unwrap();
        store
            .compact_before(now - time::Duration::days(90))
            .unwrap();
        let totals = store
            .token_totals_by_source_between(old - time::Duration::minutes(10), now)
            .unwrap();
        let sum: f64 = totals.iter().map(|(_, v)| v).sum();
        assert_eq!(
            sum, 0.0,
            "both-axes-old rows must move into daily_aggregates"
        );
        assert_eq!(
            store
                .daily_aggregate_effective_tokens("claude-code")
                .unwrap(),
            4_444.0,
            "compacted row must roll up into daily_aggregates"
        );
    }

    #[test]
    fn migrate_creates_bucket_at_index() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_usage_events_bucket_at'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn applied_effective_tokens_between_excludes_unapplied_rows() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store.insert_event(&sample_event_at(now, 1_000.0)).unwrap(); // applied
        let staged = NormalizedUsageEvent {
            observed_at: now,
            bucket_at: now,
            ..NormalizedUsageEvent::for_test_at(now, 500.0)
        };
        store
            .insert_unapplied_event_bucket(
                &staged,
                &ProviderCursorUpdate {
                    provider_surface: "claude-code".into(),
                    cursor_key: "k".into(),
                    cursor_value: "v".into(),
                    provider_version: "p".into(),
                    parser_version: "q".into(),
                },
                0,
                1,
            )
            .unwrap();
        let sum = store
            .applied_effective_tokens_between(
                now - time::Duration::hours(1),
                now + time::Duration::seconds(1),
            )
            .unwrap();
        assert_eq!(
            sum, 1_000.0,
            "staged rows must not leak into DayContext reads"
        );
    }

    #[test]
    fn applied_effective_tokens_between_is_half_open() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let start = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let end = start + time::Duration::hours(24);
        store.insert_event(&sample_event_at(start, 1.0)).unwrap(); // == start: in
        store.insert_event(&sample_event_at(end, 2.0)).unwrap(); // == end: out
        assert_eq!(
            store.applied_effective_tokens_between(start, end).unwrap(),
            1.0
        );
    }

    #[test]
    fn applied_bucket_sums_between_groups_by_bucket() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let t0 = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store.insert_event(&sample_event_at(t0, 100.0)).unwrap();
        store.insert_event(&sample_event_at(t0, 50.0)).unwrap(); // same bucket
        store
            .insert_event(&sample_event_at(t0 + time::Duration::minutes(10), 25.0))
            .unwrap();
        let sums = store
            .applied_bucket_sums_between(
                t0 - time::Duration::minutes(10),
                t0 + time::Duration::hours(1),
            )
            .unwrap();
        assert_eq!(sums.len(), 2);
        assert_eq!(sums[0].1, 150.0);
        assert_eq!(sums[1].1, 25.0);
    }

    #[test]
    fn applied_token_shape_between_sums_components() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let mut event = sample_event_at(now, 1_030.0);
        event.input_tokens = 500.0;
        event.output_tokens = 400.0;
        event.cache_creation_tokens = 100.0;
        event.cache_read_tokens = 1_000.0; // stored effective_tokens is 1030 assuming 0.03 cache-read weighting
        event.reasoning_output_tokens = 0.0;
        store.insert_event(&event).unwrap();
        let shape = store
            .applied_token_shape_between(
                now - time::Duration::hours(1),
                now + time::Duration::seconds(1),
            )
            .unwrap();
        assert_eq!(shape.input_tokens, 500.0);
        assert_eq!(shape.output_tokens, 400.0);
        assert_eq!(shape.cache_creation_tokens, 100.0);
        assert_eq!(shape.cache_read_tokens, 1_000.0);
        assert_eq!(shape.reasoning_output_tokens, 0.0);
        assert_eq!(shape.effective_tokens, 1_030.0);
    }

    #[test]
    fn latest_applied_bucket_at_and_existence_probes() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        assert!(!store.has_any_applied_events().unwrap());
        assert_eq!(store.latest_applied_bucket_at().unwrap(), None);
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store.insert_event(&sample_event_at(now, 1.0)).unwrap();
        store
            .insert_event(&sample_event_at(now - time::Duration::hours(2), 1.0))
            .unwrap();
        assert!(store.has_any_applied_events().unwrap());
        assert_eq!(store.latest_applied_bucket_at().unwrap(), Some(now));
        assert_eq!(
            store.latest_applied_bucket_at_before(now).unwrap(),
            Some(now - time::Duration::hours(2))
        );
    }

    #[test]
    fn latest_applied_marked_at_returns_max_applied_at() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        assert_eq!(store.latest_applied_marked_at().unwrap(), None);
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store.insert_event(&sample_event_at(now, 1.0)).unwrap();
        assert_eq!(store.latest_applied_marked_at().unwrap(), Some(now));
    }

    #[test]
    fn applied_effective_tokens_between_returns_zero_when_empty() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        assert_eq!(
            store
                .applied_effective_tokens_between(now - time::Duration::hours(1), now,)
                .unwrap(),
            0.0
        );
    }

    #[test]
    fn applied_bucket_sums_between_returns_empty_when_empty() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        assert!(store
            .applied_bucket_sums_between(now - time::Duration::hours(1), now,)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn applied_token_shape_between_returns_default_when_empty() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let shape = store
            .applied_token_shape_between(now - time::Duration::hours(1), now)
            .unwrap();
        assert_eq!(shape, AppliedShapeSums::default());
    }

    #[test]
    fn has_any_applied_events_false_with_only_unapplied_rows() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let staged = sample_event_at(now, 500.0);
        store
            .insert_unapplied_event_bucket(
                &staged,
                &ProviderCursorUpdate {
                    provider_surface: "claude-code".into(),
                    cursor_key: "k".into(),
                    cursor_value: "v".into(),
                    provider_version: "p".into(),
                    parser_version: "q".into(),
                },
                0,
                1,
            )
            .unwrap();
        assert!(!store.has_any_applied_events().unwrap());
    }

    #[test]
    fn applied_effective_tokens_by_source_between_groups_applied_rows_per_surface() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let mut codex = sample_event_at(now, 3_000.0);
        codex.provider_surface = "codex".into();
        store.insert_event(&codex).unwrap();
        store.insert_event(&sample_event_at(now, 1_000.0)).unwrap(); // claude-code
        let totals = store
            .applied_effective_tokens_by_source_between(
                now - time::Duration::hours(1),
                now + time::Duration::seconds(1),
            )
            .unwrap();
        assert_eq!(totals.len(), 2);
        assert!(totals.contains(&("codex".to_string(), 3_000.0)));
        assert!(totals.contains(&("claude-code".to_string(), 1_000.0)));
    }

    #[test]
    fn latest_applied_bucket_at_before_returns_none_when_no_earlier_bucket() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store.insert_event(&sample_event_at(now, 1.0)).unwrap();
        assert_eq!(store.latest_applied_bucket_at_before(now).unwrap(), None);
    }

    fn contact_update(surface: &str, key: &str) -> ProviderCursorUpdate {
        ProviderCursorUpdate {
            provider_surface: surface.to_string(),
            cursor_key: key.to_string(),
            cursor_value: "seeded".to_string(),
            provider_version: "test-provider".to_string(),
            parser_version: "test-parser".to_string(),
        }
    }

    #[test]
    fn latest_cursor_updated_at_is_none_without_cursors_and_max_per_surface() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        assert_eq!(store.latest_cursor_updated_at("claude-code").unwrap(), None);

        let early = datetime!(2026-06-08 09:00 UTC);
        let late = datetime!(2026-06-09 21:00 UTC);
        store
            .advance_cursors(vec![contact_update("claude-code", "cursor-a")], early)
            .unwrap();
        store
            .advance_cursors(vec![contact_update("claude-code", "cursor-b")], late)
            .unwrap();
        store
            .advance_cursors(vec![contact_update("codex", "cursor-c")], early)
            .unwrap();

        assert_eq!(
            store.latest_cursor_updated_at("claude-code").unwrap(),
            Some(late)
        );
        assert_eq!(
            store.latest_cursor_updated_at("codex").unwrap(),
            Some(early)
        );
        assert_eq!(store.latest_cursor_updated_at("gemini").unwrap(), None);
    }

    #[test]
    fn refuse_poll_discontinuity_advances_cursors_and_persists_diagnostic_together() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = datetime!(2026-06-10 08:00 UTC);
        let diagnostic = ProviderDiagnostic {
            provider_surface: "claude-code".to_string(),
            code: "usage_discontinuity".to_string(),
            message: "refused 212000000 effective tokens (threshold 99000000)".to_string(),
            recorded_at: now,
        };

        store
            .refuse_poll_discontinuity(
                vec![contact_update("claude-code", "cursor-a")],
                &diagnostic,
                now,
            )
            .unwrap();

        // Cursors advanced...
        assert_eq!(
            store.latest_cursor_updated_at("claude-code").unwrap(),
            Some(now)
        );
        assert_eq!(
            store
                .provider_cursor("claude-code", "cursor-a")
                .unwrap()
                .as_deref(),
            Some("seeded")
        );
        // ...the diagnostic persisted...
        let stored = store.recent_diagnostics(5).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].provider_surface, "claude-code");
        assert_eq!(stored[0].code, "usage_discontinuity");
        assert_eq!(stored[0].recorded_at, now);
        // ...and nothing was staged: a refusal never creates food.
        assert_eq!(store.unapplied_events(10).unwrap().len(), 0);
        assert_eq!(store.recent_event_count().unwrap(), 0);
    }

    #[test]
    fn refuse_poll_discontinuity_is_one_transaction() {
        // Atomicity: the diagnostic is present iff the cursors advanced. Drop
        // the diagnostics table so the LAST statement in the transaction
        // fails — a non-transactional implementation would leave the cursor
        // upserts behind, silently discarding tokens with no record.
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = datetime!(2026-06-10 08:00 UTC);
        store
            .conn
            .execute("DROP TABLE provider_diagnostics", [])
            .unwrap();
        let diagnostic = ProviderDiagnostic {
            provider_surface: "claude-code".to_string(),
            code: "usage_discontinuity".to_string(),
            message: "refused".to_string(),
            recorded_at: now,
        };

        let result = store.refuse_poll_discontinuity(
            vec![contact_update("claude-code", "cursor-a")],
            &diagnostic,
            now,
        );

        assert!(
            result.is_err(),
            "a failed diagnostic insert must fail the whole call"
        );
        assert_eq!(
            store.latest_cursor_updated_at("claude-code").unwrap(),
            None,
            "cursor upserts must roll back with the failed diagnostic insert"
        );
    }

    #[test]
    fn seed_source_history_writes_applied_rows_and_cursors_without_feeding_lifetime() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let historical = now - time::Duration::days(2);

        let event = NormalizedUsageEvent {
            provider_surface: "gemini".into(),
            effective_tokens: 50_000.0,
            ..NormalizedUsageEvent::for_test_at(historical, 50_000.0)
        };
        let cursor = ProviderCursorUpdate {
            provider_surface: "gemini".into(),
            cursor_key: "gemini|daily|2026-06-09".into(),
            cursor_value: "totals-v1".into(),
            provider_version: "ccusage 20.0.6".into(),
            parser_version: "ccusage 20.0.6".into(),
        };

        store
            .seed_source_history(&[(event, cursor)], None, now)
            .unwrap();

        assert_eq!(store.lifetime_effective_tokens().unwrap(), 0.0);
        assert!(!store.has_any_applied_events().unwrap());
        assert_eq!(
            store
                .provider_cursor("gemini", "gemini|daily|2026-06-09")
                .unwrap()
                .as_deref(),
            Some("totals-v1")
        );
        assert!(store
            .applied_effective_tokens_by_source_between(
                historical - time::Duration::hours(1),
                now + time::Duration::seconds(1),
            )
            .unwrap()
            .is_empty());
    }

    #[test]
    fn seed_source_history_rows_are_non_feedable_for_activity_queries() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = datetime!(2026 - 06 - 10 12:00 UTC);
        let historical = now - time::Duration::days(1);
        let event = NormalizedUsageEvent {
            provider_surface: "claude-code".into(),
            ..NormalizedUsageEvent::for_test_at(historical, 50_000.0)
        };
        let cursor = ProviderCursorUpdate {
            provider_surface: "claude-code".into(),
            cursor_key: "seed-key".into(),
            cursor_value: "seed-value".into(),
            provider_version: "test-provider".into(),
            parser_version: "test-parser".into(),
        };

        store
            .seed_source_history(&[(event, cursor)], None, now)
            .unwrap();

        assert_eq!(store.lifetime_effective_tokens().unwrap(), 0.0);
        assert_eq!(store.recent_event_count().unwrap(), 0);
        assert!(!store.has_any_applied_events().unwrap());
        assert_eq!(
            store
                .applied_effective_tokens_between(historical - time::Duration::hours(1), now)
                .unwrap(),
            0.0
        );
        assert!(store
            .applied_effective_tokens_by_source_between(historical - time::Duration::hours(1), now)
            .unwrap()
            .is_empty());
        assert!(store.recent_events(10).unwrap().is_empty());
    }

    #[test]
    fn feedable_applied_rows_still_drive_activity_queries() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = datetime!(2026 - 06 - 10 12:00 UTC);
        store
            .insert_event(&NormalizedUsageEvent::for_test_at(now, 42_000.0))
            .unwrap();

        assert_eq!(store.recent_event_count().unwrap(), 1);
        assert!(store.has_any_applied_events().unwrap());
        assert_eq!(
            store
                .applied_effective_tokens_between(
                    now - time::Duration::hours(1),
                    now + time::Duration::seconds(1)
                )
                .unwrap(),
            42_000.0
        );
    }

    #[test]
    fn migrate_backfills_legacy_seed_rows_without_touching_real_applied_rows() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("usage.sqlite");
        let seeded_at = datetime!(2026-06-10 12:00 UTC);
        let historical = seeded_at - time::Duration::days(1);
        let live_applied_at = seeded_at + time::Duration::hours(3);
        let seeded_cursor_key = "gemini|daily|2026-06-09";
        let seeded_cursor_value = "totals-v1";
        let live_cursor_key = "gemini|daily|2026-06-10";
        let live_cursor_value = "totals-v2";

        {
            let conn = Connection::open(&db).unwrap();
            conn.execute_batch(
                "
                CREATE TABLE usage_events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    provider_surface TEXT NOT NULL,
                    provider_version TEXT NOT NULL,
                    parser_version TEXT NOT NULL,
                    command TEXT NOT NULL,
                    source_surface TEXT NOT NULL,
                    period_start TEXT NOT NULL,
                    observed_at TEXT NOT NULL,
                    bucket_at TEXT NOT NULL,
                    period_date TEXT NOT NULL,
                    model TEXT,
                    input_tokens REAL NOT NULL,
                    output_tokens REAL NOT NULL,
                    cache_creation_tokens REAL NOT NULL,
                    cache_read_tokens REAL NOT NULL,
                    reasoning_output_tokens REAL NOT NULL,
                    effective_tokens REAL NOT NULL,
                    total_tokens REAL NOT NULL DEFAULT 0.0,
                    token_contract TEXT NOT NULL DEFAULT 'weighted_effective_v1',
                    cost_usd REAL,
                    confidence TEXT NOT NULL,
                    provider_delta_id TEXT,
                    bucket_index INTEGER NOT NULL DEFAULT 0,
                    bucket_count INTEGER NOT NULL DEFAULT 1,
                    applied_at TEXT,
                    provider_cursor_key TEXT,
                    provider_cursor_value TEXT
                );

                CREATE TABLE provider_diagnostics (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    provider_surface TEXT NOT NULL,
                    code TEXT NOT NULL,
                    message TEXT NOT NULL,
                    recorded_at TEXT NOT NULL
                );
                ",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO usage_events (
                    provider_surface, provider_version, parser_version, command, source_surface,
                    period_start, observed_at, bucket_at, period_date, model,
                    input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens,
                    reasoning_output_tokens, effective_tokens, total_tokens, token_contract,
                    cost_usd, confidence,
                    provider_delta_id, bucket_index, bucket_count, applied_at,
                    provider_cursor_key, provider_cursor_value
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                    ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                    ?21, ?22, ?23, ?24, ?25, ?26
                )",
                params![
                    "gemini",
                    "test-provider",
                    "test-parser",
                    "ccusage daily --json --offline",
                    "daily",
                    format_time(historical).unwrap(),
                    format_time(seeded_at).unwrap(),
                    format_time(historical).unwrap(),
                    historical.date().to_string(),
                    Option::<String>::None,
                    50_000.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    50_000.0,
                    50_000.0,
                    crate::usage::token_contract::WEIGHTED_EFFECTIVE_V1,
                    Option::<f64>::None,
                    "local-log-derived",
                    format!("gemini|{seeded_cursor_key}|{seeded_cursor_value}"),
                    0_i64,
                    1_i64,
                    format_time(seeded_at).unwrap(),
                    seeded_cursor_key,
                    seeded_cursor_value,
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO usage_events (
                    provider_surface, provider_version, parser_version, command, source_surface,
                    period_start, observed_at, bucket_at, period_date, model,
                    input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens,
                    reasoning_output_tokens, effective_tokens, total_tokens, token_contract,
                    cost_usd, confidence,
                    provider_delta_id, bucket_index, bucket_count, applied_at,
                    provider_cursor_key, provider_cursor_value
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                    ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                    ?21, ?22, ?23, ?24, ?25, ?26
                )",
                params![
                    "gemini",
                    "test-provider",
                    "test-parser",
                    "ccusage daily --json --offline",
                    "daily",
                    format_time(live_applied_at).unwrap(),
                    format_time(live_applied_at).unwrap(),
                    format_time(live_applied_at).unwrap(),
                    live_applied_at.date().to_string(),
                    Option::<String>::None,
                    42_000.0,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    42_000.0,
                    42_000.0,
                    crate::usage::token_contract::WEIGHTED_EFFECTIVE_V1,
                    Option::<f64>::None,
                    "local-log-derived",
                    format!("gemini|{live_cursor_key}|{live_cursor_value}"),
                    0_i64,
                    1_i64,
                    format_time(live_applied_at).unwrap(),
                    live_cursor_key,
                    live_cursor_value,
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO provider_diagnostics (
                    provider_surface, code, message, recorded_at
                ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    "gemini",
                    "source_first_contact",
                    "first contact with gemini: 1 historical rows seeded without feeding",
                    format_time(seeded_at).unwrap(),
                ],
            )
            .unwrap();
        }

        let store = UsageStore::open(&db).unwrap();
        let rows: Vec<(String, i64)> = {
            let mut stmt = store
                .conn
                .prepare(
                    "SELECT provider_cursor_value, feedable
                     FROM usage_events
                     ORDER BY applied_at ASC, id ASC",
                )
                .unwrap();
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };

        assert_eq!(
            rows,
            vec![
                (seeded_cursor_value.to_string(), 0_i64),
                (live_cursor_value.to_string(), 1_i64),
            ]
        );
        assert_eq!(store.recent_event_count().unwrap(), 1);
        assert_eq!(
            store
                .applied_effective_tokens_between(
                    historical - time::Duration::hours(1),
                    live_applied_at + time::Duration::seconds(1),
                )
                .unwrap(),
            42_000.0
        );
    }

    #[test]
    fn migrate_moves_data_cursor_partition_to_source_label() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let key = r#"{"provider_surface":"gemini","command":"ccusage","source_surface":"daily","period_start":"2026-06-11","model":null}"#;

        store
            .set_provider_cursor("ccusage", key, "v1", "20.0.6", "20.0.6")
            .unwrap();

        // Re-run migrations as an upgrade would.
        store.migrate().unwrap();

        assert_eq!(
            store.provider_cursor("gemini", key).unwrap().as_deref(),
            Some("v1")
        );
        assert_eq!(store.provider_cursor("ccusage", key).unwrap(), None);
    }

    #[test]
    fn migrate_deletes_stale_helper_surface_row_on_conflict() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let key = r#"{"provider_surface":"gemini","command":"ccusage","source_surface":"daily","period_start":"2026-06-11","model":null}"#;

        // Seed both the legacy helper-surface cursor and an already-migrated
        // source-label cursor with the same key.
        store
            .set_provider_cursor("ccusage", key, "v1", "20.0.6", "20.0.6")
            .unwrap();
        store
            .set_provider_cursor("gemini", key, "v2", "20.0.6", "20.0.6")
            .unwrap();

        store.migrate().unwrap();

        assert_eq!(
            store.provider_cursor("gemini", key).unwrap().as_deref(),
            Some("v2")
        );
        assert_eq!(store.provider_cursor("ccusage", key).unwrap(), None);
    }
}
