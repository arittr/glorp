use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

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
                cost_usd,
                confidence,
                applied_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19
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
                event.cost_usd,
                event.confidence,
                format_time(event.observed_at)?,
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
                cost_usd,
                confidence,
                provider_delta_id,
                bucket_index,
                bucket_count,
                applied_at,
                provider_cursor_key,
                provider_cursor_value
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                ?19, ?20, ?21, NULL, ?22, ?23
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
                event.cost_usd,
                event.confidence,
                provider_delta_id,
                bucket_index_i64,
                bucket_count_i64,
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
            WHERE period_start < ?1 AND bucket_at < ?1 AND applied_at IS NOT NULL
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
            "DELETE FROM usage_events WHERE period_start < ?1 AND bucket_at < ?1 AND applied_at IS NOT NULL",
            params![format_time(cutoff)?],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn recent_event_count(&self) -> crate::error::Result<u64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM usage_events", [], |row| row.get(0))
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
                cost_usd,
                confidence,
                provider_delta_id
             FROM usage_events
             WHERE observed_at >= ?1
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
                    cost_usd: row.get(15)?,
                    confidence: row.get(16)?,
                    provider_delta_id: row.get(17)?,
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
                cost_usd,
                confidence,
                provider_delta_id
             FROM usage_events
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
                    cost_usd: row.get(15)?,
                    confidence: row.get(16)?,
                    provider_delta_id: row.get(17)?,
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
                cost_usd,
                confidence,
                provider_cursor_key,
                provider_cursor_value,
                provider_delta_id
             FROM usage_events
             WHERE applied_at IS NULL
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
                let cursor_key: Option<String> = row.get(18)?;
                let cursor_value: Option<String> = row.get(19)?;
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
                    cost_usd: row.get(16)?,
                    confidence: row.get(17)?,
                    provider_delta_id: row.get(20)?,
                };
                let cursor_update = ProviderCursorUpdate {
                    provider_surface,
                    cursor_key: cursor_key.unwrap_or_default(),
                    cursor_value: cursor_value.unwrap_or_default(),
                    provider_version,
                    parser_version,
                };
                Ok(UsageLedgerRow {
                    id: row.get(0)?,
                    event,
                    cursor_update,
                })
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
                 WHERE id IN ({placeholders}) AND applied_at IS NULL"
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
             WHERE id IN ({placeholders}) AND applied_at IS NULL"
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
             WHERE bucket_at >= ?1 AND bucket_at <= ?2
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
                 WHERE applied_at IS NOT NULL AND bucket_at >= ?1 AND bucket_at < ?2",
                params![format_time(start)?, format_time(end)?],
                |row| row.get(0),
            )
            .map_err(Into::into)
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
             WHERE applied_at IS NOT NULL AND bucket_at >= ?1 AND bucket_at < ?2
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
                 WHERE applied_at IS NOT NULL AND bucket_at >= ?1 AND bucket_at < ?2",
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
            "SELECT MAX(bucket_at) FROM usage_events WHERE applied_at IS NOT NULL",
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
            "SELECT MAX(applied_at) FROM usage_events WHERE applied_at IS NOT NULL",
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
             WHERE applied_at IS NOT NULL AND bucket_at < ?1",
            params![format_time(at)?],
            |row| row.get(0),
        )?;
        max.map(|s| parse_time_for_sql(&s).map_err(Into::into))
            .transpose()
    }

    /// Whether the pet has ever eaten (any applied row). Newborn sleep gate.
    pub fn has_any_applied_events(&self) -> crate::error::Result<bool> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM usage_events WHERE applied_at IS NOT NULL)",
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
                cost_usd REAL,
                confidence TEXT NOT NULL,
                provider_delta_id TEXT,
                bucket_index INTEGER NOT NULL DEFAULT 0,
                bucket_count INTEGER NOT NULL DEFAULT 1,
                applied_at TEXT,
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
            ",
        )?;
        Ok(())
    }
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
}
