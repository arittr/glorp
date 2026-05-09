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
                period_date,
                model,
                input_tokens,
                output_tokens,
                cache_creation_tokens,
                cache_read_tokens,
                reasoning_output_tokens,
                effective_tokens,
                cost_usd,
                confidence
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                event.provider_surface,
                event.provider_version,
                event.parser_version,
                event.command,
                event.source_surface,
                format_time(event.period_start)?,
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
            ],
        )?;
        add_lifetime_counter(&tx, event.effective_tokens)?;
        tx.commit()?;
        Ok(())
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
            WHERE period_start < ?1
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
            "DELETE FROM usage_events WHERE period_start < ?1",
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
             ORDER BY period_start DESC, id DESC
             LIMIT ?1",
        )?;
        let events = stmt
            .query_map(params![limit], |row| {
                let period_start: String = row.get(5)?;
                Ok(NormalizedUsageEvent {
                    provider_surface: row.get(0)?,
                    provider_version: row.get(1)?,
                    parser_version: row.get(2)?,
                    command: row.get(3)?,
                    source_surface: row.get(4)?,
                    period_start: parse_time_for_sql(&period_start)?,
                    model: row.get(6)?,
                    input_tokens: row.get(7)?,
                    output_tokens: row.get(8)?,
                    cache_creation_tokens: row.get(9)?,
                    cache_read_tokens: row.get(10)?,
                    reasoning_output_tokens: row.get(11)?,
                    effective_tokens: row.get(12)?,
                    cost_usd: row.get(13)?,
                    confidence: row.get(14)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(events)
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
                period_date TEXT NOT NULL,
                model TEXT,
                input_tokens REAL NOT NULL,
                output_tokens REAL NOT NULL,
                cache_creation_tokens REAL NOT NULL,
                cache_read_tokens REAL NOT NULL,
                reasoning_output_tokens REAL NOT NULL,
                effective_tokens REAL NOT NULL,
                cost_usd REAL,
                confidence TEXT NOT NULL
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
        Ok(())
    }
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
