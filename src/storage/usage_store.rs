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

    pub fn insert_unapplied_event(
        &mut self,
        event: &NormalizedUsageEvent,
        cursor_update: &ProviderCursorUpdate,
    ) -> crate::error::Result<i64> {
        let provider_delta_id = format!(
            "{}|{}|{}",
            cursor_update.provider_surface, cursor_update.cursor_key, cursor_update.cursor_value
        );
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
                ?19, 0, 1, NULL, ?20, ?21
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
                cursor_update.cursor_key,
                cursor_update.cursor_value,
            ],
        )?;
        let id: i64 = tx.query_row(
            "SELECT id FROM usage_events
             WHERE provider_delta_id = ?1 AND bucket_index = ?2",
            params![provider_delta_id, 0_i64],
            |row| row.get(0),
        )?;
        tx.commit()?;
        Ok(id)
    }

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
            WHERE period_start < ?1 AND applied_at IS NOT NULL
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
            "DELETE FROM usage_events WHERE period_start < ?1 AND applied_at IS NOT NULL",
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
                confidence
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
                provider_cursor_value
             FROM usage_events
             WHERE applied_at IS NULL
             ORDER BY observed_at ASC, id ASC
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
                    applied_at_text,
                ],
            )?;
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

    pub fn today_effective_tokens(&self) -> crate::error::Result<f64> {
        let today = OffsetDateTime::now_utc().date().to_string();
        self.conn
            .query_row(
                "SELECT COALESCE(SUM(effective_tokens), 0.0)
                 FROM usage_events
                 WHERE period_date = ?1",
                params![today],
                |row| row.get(0),
            )
            .map_err(Into::into)
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
