use crate::error::{GlorpError, Result};
use crate::storage::usage_store::{
    ProviderCursorUpdate, ProviderDiagnostic as StoredProviderDiagnostic, UsageStore,
};
use crate::usage::ccusage::{
    requested_provider_days_for_poll, run_command_with_timeout, stable_hash, HelperCommand,
    HELPER_SUBPROCESS_TIMEOUT,
};
use crate::usage::day_axis::{
    parse_agentsview_period_date, tokenmaxxing_day_start, tokenmaxxing_provider_day,
    TOKENMAXXING_TIMEZONE,
};
use crate::usage::normalize::{normalize_agentsview_json, NormalizedUsageRecord};
use crate::usage::provider::{
    ProviderCursorKey, ProviderDiagnostic, ProviderSnapshotScope, UsageDelta, UsagePollResult,
    UsageProvider, UsageSnapshot,
};
use crate::usage::snapshot::{
    ProviderSnapshotBatchInput, ProviderSnapshotDiagnosticInput, ProviderSnapshotRowInput,
};
use crate::usage::token_contract::TOKENMAXXING_TOTAL_V1;
use serde_json::Value;
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use time::{Date, OffsetDateTime};

pub const AGENTSVIEW_COMMAND: &str = "agentsview usage daily";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentsviewPaths {
    pub agentsview: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentsviewDiscovery {
    pub agentsview: Option<PathBuf>,
}

#[derive(Clone)]
pub struct AgentsviewCommandProvider {
    paths: AgentsviewPaths,
    clock: Arc<dyn Fn() -> OffsetDateTime + Send + Sync>,
}

#[derive(Debug)]
struct AgentSnapshotFlow {
    result: UsagePollResult,
}

#[derive(Debug)]
struct PreparedAgentSnapshot {
    scope: ProviderSnapshotScope,
    collector_surface: String,
    command_name: String,
    version: String,
    rows: Vec<ProviderSnapshotRowInput>,
    diagnostics: Vec<ProviderDiagnostic>,
    outcome: AgentSnapshotOutcome,
    helper_configured: bool,
    covered_accounting_sources: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
struct UnexpectedProviderDayDiagnostic {
    diagnostic: ProviderDiagnostic,
    provider_day: Date,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentSnapshotOutcome {
    Completed,
    Blocked,
}

impl fmt::Debug for AgentsviewCommandProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentsviewCommandProvider")
            .field("paths", &self.paths)
            .finish_non_exhaustive()
    }
}

impl AgentsviewCommandProvider {
    pub fn new(paths: AgentsviewPaths) -> Self {
        Self::new_with_clock(paths, crate::time::usage_now_utc)
    }

    fn new_with_clock<F>(paths: AgentsviewPaths, clock: F) -> Self
    where
        F: Fn() -> OffsetDateTime + Send + Sync + 'static,
    {
        Self { paths, clock: Arc::new(clock) }
    }

    #[doc(hidden)]
    pub fn new_with_now_for_test(paths: AgentsviewPaths, now: OffsetDateTime) -> Self {
        Self::new_with_clock(paths, move || now)
    }

    pub fn from_environment() -> Self {
        Self::new(AgentsviewDiscovery::discover().into())
    }

    pub fn discovered_version(&self) -> Option<String> {
        let helper = self.paths.agentsview.as_deref()?;
        if !helper.exists() {
            return None;
        }
        self.version(helper)
    }

    pub fn poll_current_day(&self, store: &mut UsageStore) -> Result<UsagePollResult> {
        let now = (self.clock)();
        let date = tokenmaxxing_provider_day(now).to_string();
        self.poll_combined_agents(
            store,
            UsageRange::CurrentDay { since: date.clone(), until: date },
            true,
        )
    }

    pub fn poll_full_history(&self, store: &mut UsageStore) -> Result<UsagePollResult> {
        self.poll_combined_agents(store, UsageRange::FullHistory, true)
    }

    #[cfg(test)]
    fn poll_agent_with_timeout(
        &self,
        store: &mut UsageStore,
        agent: &str,
        no_sync: bool,
        range: UsageRange,
        timeout: Duration,
    ) -> Result<UsagePollResult> {
        Ok(self
            .poll_agent_flow_with_timeout(store, agent, no_sync, range, timeout, true)?
            .result)
    }

    #[cfg(test)]
    fn poll_agent_flow_with_timeout(
        &self,
        store: &mut UsageStore,
        agent: &str,
        no_sync: bool,
        range: UsageRange,
        timeout: Duration,
        feed: bool,
    ) -> Result<AgentSnapshotFlow> {
        let requested_provider_days = requested_provider_days_for_poll((self.clock)());
        let scope = snapshot_scope(agent, requested_provider_days);
        let prepared = self.prepare_agent_snapshot(store, agent, no_sync, range, timeout, scope)?;
        self.finish_prepared_agent_snapshot(store, prepared, feed)
    }

    fn prepare_agent_snapshot(
        &self,
        store: &mut UsageStore,
        agent: &str,
        no_sync: bool,
        range: UsageRange,
        timeout: Duration,
        scope: ProviderSnapshotScope,
    ) -> Result<PreparedAgentSnapshot> {
        let helper_configured = self.paths.agentsview.is_some();
        let observed_at = OffsetDateTime::now_utc();
        let Some(helper) = self.paths.agentsview.as_deref() else {
            let diagnostic = diagnostic(agent, "missing_helper", "agentsview helper was not found");
            persist_diagnostic(store, &diagnostic)?;
            record_snapshot_failures(
                store,
                &scope,
                agent,
                std::slice::from_ref(&diagnostic),
                observed_at,
            )?;
            return Ok(PreparedAgentSnapshot {
                scope,
                collector_surface: collector_surface(agent),
                command_name: AGENTSVIEW_COMMAND.to_string(),
                version: "unknown".to_string(),
                rows: Vec::new(),
                diagnostics: vec![diagnostic],
                outcome: AgentSnapshotOutcome::Blocked,
                helper_configured,
                covered_accounting_sources: Some(vec![agent.to_string()]),
            });
        };
        if !helper.exists() {
            let diagnostic = diagnostic(agent, "missing_helper", "agentsview helper was not found");
            persist_diagnostic(store, &diagnostic)?;
            record_snapshot_failures(
                store,
                &scope,
                agent,
                std::slice::from_ref(&diagnostic),
                observed_at,
            )?;
            return Ok(PreparedAgentSnapshot {
                scope,
                collector_surface: collector_surface(agent),
                command_name: AGENTSVIEW_COMMAND.to_string(),
                version: "unknown".to_string(),
                rows: Vec::new(),
                diagnostics: vec![diagnostic],
                outcome: AgentSnapshotOutcome::Blocked,
                helper_configured,
                covered_accounting_sources: Some(vec![agent.to_string()]),
            });
        }

        let version = self
            .version_with_timeout(helper, timeout)
            .unwrap_or_else(|| "unknown".to_string());
        let output = match self.run_usage_with_timeout(helper, agent, no_sync, range, timeout) {
            Ok(output) => output,
            Err(GlorpError::Io(err)) if err.kind() == std::io::ErrorKind::TimedOut => {
                let diagnostic = diagnostic(
                    agent,
                    "helper_timeout",
                    &format!(
                        "agentsview did not return within {}s",
                        HELPER_SUBPROCESS_TIMEOUT.as_secs()
                    ),
                );
                persist_diagnostic(store, &diagnostic)?;
                record_snapshot_failures(
                    store,
                    &scope,
                    agent,
                    std::slice::from_ref(&diagnostic),
                    observed_at,
                )?;
                return Ok(PreparedAgentSnapshot {
                    scope,
                    collector_surface: collector_surface(agent),
                    command_name: AGENTSVIEW_COMMAND.to_string(),
                    version,
                    rows: Vec::new(),
                    diagnostics: vec![diagnostic],
                    outcome: AgentSnapshotOutcome::Blocked,
                    helper_configured,
                    covered_accounting_sources: Some(vec![agent.to_string()]),
                });
            }
            Err(err) => return Err(err),
        };
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let diagnostic = diagnostic(
                agent,
                "helper_exit",
                &format!("agentsview exited with status {code}"),
            );
            persist_diagnostic(store, &diagnostic)?;
            record_snapshot_failures(
                store,
                &scope,
                agent,
                std::slice::from_ref(&diagnostic),
                observed_at,
            )?;
            return Ok(PreparedAgentSnapshot {
                scope,
                collector_surface: collector_surface(agent),
                command_name: AGENTSVIEW_COMMAND.to_string(),
                version,
                rows: Vec::new(),
                diagnostics: vec![diagnostic],
                outcome: AgentSnapshotOutcome::Blocked,
                helper_configured,
                covered_accounting_sources: Some(vec![agent.to_string()]),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let (stdout, unexpected_provider_days) = match filter_agentsview_json_to_requested_days(
            agent,
            &stdout,
            &scope.requested_provider_days,
        ) {
            Ok(filtered) => filtered,
            Err(diagnostic) => {
                persist_diagnostic(store, &diagnostic)?;
                record_snapshot_failures(
                    store,
                    &scope,
                    agent,
                    std::slice::from_ref(&diagnostic),
                    observed_at,
                )?;
                return Ok(PreparedAgentSnapshot {
                    scope,
                    collector_surface: collector_surface(agent),
                    command_name: AGENTSVIEW_COMMAND.to_string(),
                    version,
                    rows: Vec::new(),
                    diagnostics: vec![diagnostic],
                    outcome: AgentSnapshotOutcome::Blocked,
                    helper_configured,
                    covered_accounting_sources: Some(vec![agent.to_string()]),
                });
            }
        };
        let mut diagnostics = Vec::new();
        for unexpected in &unexpected_provider_days {
            persist_diagnostic(store, &unexpected.diagnostic)?;
            record_snapshot_diagnostic(
                store,
                &scope,
                None,
                Some(unexpected.provider_day),
                "unexpected_provider_day",
                "unexpected_provider_day",
                &unexpected.diagnostic.message,
                observed_at,
            )?;
            diagnostics.push(unexpected.diagnostic.clone());
        }
        let batch = match normalize_agentsview_json(agent, &stdout) {
            Ok(batch) => batch,
            Err(diagnostic) => {
                persist_diagnostic(store, &diagnostic)?;
                record_snapshot_failures(
                    store,
                    &scope,
                    agent,
                    std::slice::from_ref(&diagnostic),
                    observed_at,
                )?;
                return Ok(PreparedAgentSnapshot {
                    scope,
                    collector_surface: collector_surface(agent),
                    command_name: AGENTSVIEW_COMMAND.to_string(),
                    version,
                    rows: Vec::new(),
                    diagnostics: vec![diagnostic],
                    outcome: AgentSnapshotOutcome::Blocked,
                    helper_configured,
                    covered_accounting_sources: Some(vec![agent.to_string()]),
                });
            }
        };
        for diagnostic in &batch.diagnostics {
            persist_diagnostic(store, diagnostic)?;
        }

        let mut rows = Vec::new();
        diagnostics.extend(batch.diagnostics.clone());
        let mut blocking_parse_failure = batch
            .diagnostics
            .iter()
            .any(|diagnostic| blocks_requested_snapshot(diagnostic.code.as_str()));
        for record in batch.records {
            let Ok((_date, period_start)) = parse_agentsview_period_date(&record.period_start)
            else {
                let diagnostic = diagnostic(
                    agent,
                    "invalid_period_start",
                    "agentsview invalid_period_start",
                );
                persist_diagnostic(store, &diagnostic)?;
                diagnostics.push(diagnostic);
                blocking_parse_failure = true;
                continue;
            };
            let provider_day = period_start.date();

            if !scope.requested_provider_days.contains(&provider_day) {
                let diagnostic = diagnostic(
                    agent,
                    "unexpected_provider_day",
                    &format!("agentsview returned unrequested provider day {provider_day}"),
                );
                persist_diagnostic(store, &diagnostic)?;
                record_snapshot_diagnostic(
                    store,
                    &scope,
                    None,
                    Some(provider_day),
                    "unexpected_provider_day",
                    "unexpected_provider_day",
                    &diagnostic.message,
                    observed_at,
                )?;
                diagnostics.push(diagnostic);
                continue;
            }

            rows.push(snapshot_row_for_record(
                &scope,
                agent,
                &version,
                record,
                provider_day,
            )?);
        }

        if blocking_parse_failure {
            let mut failure_diagnostics = diagnostics
                .iter()
                .filter(|diagnostic| blocks_requested_snapshot(diagnostic.code.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            let has_invalid_period = failure_diagnostics
                .iter()
                .any(|failure| failure.code == "invalid_period_start");
            if !has_invalid_period {
                let diagnostic = diagnostic(
                    agent,
                    "malformed_required_fields",
                    "agentsview malformed_required_fields",
                );
                persist_diagnostic(store, &diagnostic)?;
                failure_diagnostics.push(diagnostic.clone());
                diagnostics.push(diagnostic);
            }
            record_snapshot_failures(store, &scope, agent, &failure_diagnostics, observed_at)?;
            return Ok(PreparedAgentSnapshot {
                scope,
                collector_surface: collector_surface(agent),
                command_name: AGENTSVIEW_COMMAND.to_string(),
                version,
                rows: Vec::new(),
                diagnostics,
                outcome: AgentSnapshotOutcome::Blocked,
                helper_configured,
                covered_accounting_sources: Some(vec![agent.to_string()]),
            });
        }

        Ok(PreparedAgentSnapshot {
            scope,
            collector_surface: collector_surface(agent),
            command_name: AGENTSVIEW_COMMAND.to_string(),
            version,
            rows,
            diagnostics,
            outcome: AgentSnapshotOutcome::Completed,
            helper_configured,
            covered_accounting_sources: Some(vec![agent.to_string()]),
        })
    }

    fn finish_prepared_agent_snapshot(
        &self,
        store: &mut UsageStore,
        prepared: PreparedAgentSnapshot,
        feed: bool,
    ) -> Result<AgentSnapshotFlow> {
        if prepared.outcome != AgentSnapshotOutcome::Completed {
            return Ok(AgentSnapshotFlow { result: empty_poll(prepared.diagnostics) });
        }
        write_prepared_snapshot(store, &prepared)?;

        if !feed {
            return Ok(AgentSnapshotFlow { result: empty_poll(prepared.diagnostics) });
        }

        let observed_at = OffsetDateTime::now_utc();
        let plan = store.feed_deltas_for_snapshot_rows(&prepared.rows, observed_at)?;
        if !plan.cursor_seeds.is_empty() {
            store.advance_cursors(plan.cursor_seeds.clone(), observed_at)?;
        }
        let mut diagnostics = prepared.diagnostics;
        diagnostics.extend(
            plan.diagnostics
                .into_iter()
                .map(provider_diagnostic_from_stored),
        );
        Ok(AgentSnapshotFlow {
            result: result_from_parts(plan.deltas, diagnostics),
        })
    }

    fn poll_combined_agents(
        &self,
        store: &mut UsageStore,
        range: UsageRange,
        feed: bool,
    ) -> Result<UsagePollResult> {
        let requested_provider_days = requested_provider_days_for_poll((self.clock)());
        let combined_scope = ProviderSnapshotScope {
            collector_scope_id: "agentsview:scoped:local-usage".to_string(),
            replacement_scope_id: "agentsview:scoped:local-usage".to_string(),
            requested_provider_days,
        };
        let claude = self.prepare_agent_snapshot(
            store,
            "claude",
            false,
            range.clone(),
            HELPER_SUBPROCESS_TIMEOUT,
            combined_scope.clone(),
        )?;
        let codex = self.prepare_agent_snapshot(
            store,
            "codex",
            true,
            range,
            HELPER_SUBPROCESS_TIMEOUT,
            combined_scope.clone(),
        )?;

        self.finish_combined_agent_snapshots(store, vec![claude, codex], combined_scope, feed)
    }

    fn finish_combined_agent_snapshots(
        &self,
        store: &mut UsageStore,
        prepared: Vec<PreparedAgentSnapshot>,
        combined_scope: ProviderSnapshotScope,
        feed: bool,
    ) -> Result<UsagePollResult> {
        let mut diagnostics = Vec::new();
        let mut rows = Vec::new();
        let mut versions = Vec::new();
        let mut covered_sources = Vec::new();
        let mut coverage_known = true;
        let mut has_completed = false;
        let mut has_configured_blocker = false;

        for mut agent in prepared {
            diagnostics.extend(agent.diagnostics.clone());
            if agent.outcome == AgentSnapshotOutcome::Blocked && agent.helper_configured {
                has_configured_blocker = true;
            }
            if agent.outcome != AgentSnapshotOutcome::Completed {
                continue;
            }
            has_completed = true;
            versions.push(format!("{}={}", agent.collector_surface, agent.version));
            match agent.covered_accounting_sources.take() {
                Some(sources) => covered_sources.extend(sources),
                None => coverage_known = false,
            }
            rows.append(&mut agent.rows);
        }

        if has_configured_blocker || !has_completed {
            return Ok(empty_poll(diagnostics));
        }

        let version = if versions.is_empty() {
            "unknown".to_string()
        } else {
            versions.join(";")
        };
        let prepared = PreparedAgentSnapshot {
            scope: combined_scope,
            collector_surface: "agentsview:scoped".to_string(),
            command_name: AGENTSVIEW_COMMAND.to_string(),
            version,
            rows,
            diagnostics,
            outcome: AgentSnapshotOutcome::Completed,
            helper_configured: true,
            covered_accounting_sources: if coverage_known {
                covered_sources.sort();
                covered_sources.dedup();
                Some(covered_sources)
            } else {
                None
            },
        };

        Ok(self
            .finish_prepared_agent_snapshot(store, prepared, feed)?
            .result)
    }

    fn snapshot_agent_for_calibration(
        &self,
        store: &mut UsageStore,
        agent: &str,
        no_sync: bool,
    ) -> Result<UsageSnapshot> {
        let Some(helper) = self.paths.agentsview.as_deref() else {
            let diagnostic = diagnostic(agent, "missing_helper", "agentsview helper was not found");
            persist_diagnostic(store, &diagnostic)?;
            return Ok(UsageSnapshot {
                daily_usage: Vec::new(),
                cursor_updates: Vec::new(),
                diagnostics: vec![diagnostic],
            });
        };
        if !helper.exists() {
            let diagnostic = diagnostic(agent, "missing_helper", "agentsview helper was not found");
            persist_diagnostic(store, &diagnostic)?;
            return Ok(UsageSnapshot {
                daily_usage: Vec::new(),
                cursor_updates: Vec::new(),
                diagnostics: vec![diagnostic],
            });
        }

        let version = self
            .version(helper)
            .unwrap_or_else(|| "unknown".to_string());
        let output = match self.run_usage_with_timeout(
            helper,
            agent,
            no_sync,
            UsageRange::FullHistory,
            HELPER_SUBPROCESS_TIMEOUT,
        ) {
            Ok(output) => output,
            Err(GlorpError::Io(err)) if err.kind() == std::io::ErrorKind::TimedOut => {
                let diagnostic = diagnostic(
                    agent,
                    "helper_timeout",
                    &format!(
                        "agentsview did not return within {}s",
                        HELPER_SUBPROCESS_TIMEOUT.as_secs()
                    ),
                );
                persist_diagnostic(store, &diagnostic)?;
                return Ok(UsageSnapshot {
                    daily_usage: Vec::new(),
                    cursor_updates: Vec::new(),
                    diagnostics: vec![diagnostic],
                });
            }
            Err(err) => return Err(err),
        };
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let diagnostic = diagnostic(
                agent,
                "helper_exit",
                &format!("agentsview exited with status {code}"),
            );
            persist_diagnostic(store, &diagnostic)?;
            return Ok(UsageSnapshot {
                daily_usage: Vec::new(),
                cursor_updates: Vec::new(),
                diagnostics: vec![diagnostic],
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let batch = match normalize_agentsview_json(agent, &stdout) {
            Ok(batch) => batch,
            Err(diagnostic) => {
                persist_diagnostic(store, &diagnostic)?;
                return Ok(UsageSnapshot {
                    daily_usage: Vec::new(),
                    cursor_updates: Vec::new(),
                    diagnostics: vec![diagnostic],
                });
            }
        };
        for diagnostic in &batch.diagnostics {
            persist_diagnostic(store, diagnostic)?;
        }

        let mut daily_usage = Vec::new();
        let mut cursor_updates = Vec::new();
        let mut diagnostics = Vec::new();
        for record in batch.records {
            let Ok((_date, period_start)) = parse_agentsview_period_date(&record.period_start)
            else {
                let diagnostic = diagnostic(
                    agent,
                    "invalid_period_start",
                    "agentsview invalid_period_start",
                );
                persist_diagnostic(store, &diagnostic)?;
                diagnostics.push(diagnostic);
                continue;
            };
            daily_usage.push(
                crate::game::calibration::DailyUsage::with_activity_timestamp(
                    period_start,
                    record.raw_totals.total_tokens(),
                ),
            );
            cursor_updates.push(cursor_update_for_record(&record, &version)?);
        }

        diagnostics.extend(batch.diagnostics);
        Ok(UsageSnapshot { daily_usage, cursor_updates, diagnostics })
    }

    fn version(&self, helper: &Path) -> Option<String> {
        self.version_with_timeout(helper, HELPER_SUBPROCESS_TIMEOUT)
    }

    fn version_with_timeout(&self, helper: &Path, timeout: Duration) -> Option<String> {
        version_with_timeout(helper, timeout)
    }

    fn run_usage_with_timeout(
        &self,
        helper: &Path,
        agent: &str,
        no_sync: bool,
        range: UsageRange,
        timeout: Duration,
    ) -> Result<std::process::Output> {
        let hcmd = agentsview_helper_command(agent, helper, None).map_err(|d| {
            GlorpError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, d.message))
        })?;
        let mut command = Command::new(&hcmd.program);
        command.args(&hcmd.args_prefix);
        command.args([
            "usage",
            "daily",
            "--json",
            "--breakdown",
            "--agent",
            agent,
            "--timezone",
            TOKENMAXXING_TIMEZONE,
        ]);
        match range {
            UsageRange::CurrentDay { since, until } => {
                command.args(["--since", &since, "--until", &until]);
            }
            UsageRange::FullHistory => {
                command.args(["--since", "1970-01-01"]);
            }
        }
        if no_sync {
            command.arg("--no-sync");
        }
        match run_command_with_timeout(&mut command, timeout) {
            Ok(output) => Ok(output),
            Err(GlorpError::Io(err)) if err.kind() == std::io::ErrorKind::TimedOut => {
                Err(GlorpError::Io(err))
            }
            Err(err) => Err(err),
        }
    }
}

impl UsageProvider for AgentsviewCommandProvider {
    fn poll(&self, store: &mut UsageStore) -> Result<UsagePollResult> {
        self.poll_current_day(store)
    }

    fn refresh_snapshots_only(&self, store: &mut UsageStore) -> Result<Vec<ProviderDiagnostic>> {
        let now = (self.clock)();
        let date = tokenmaxxing_provider_day(now).to_string();
        Ok(self
            .poll_combined_agents(
                store,
                UsageRange::CurrentDay { since: date.clone(), until: date },
                false,
            )?
            .diagnostics)
    }

    fn snapshot_for_calibration(&self, store: &mut UsageStore) -> Result<UsageSnapshot> {
        let claude = self.snapshot_agent_for_calibration(store, "claude", false)?;
        let codex = self.snapshot_agent_for_calibration(store, "codex", true)?;
        let mut daily_usage = claude.daily_usage;
        daily_usage.extend(codex.daily_usage);
        let mut cursor_updates = claude.cursor_updates;
        cursor_updates.extend(codex.cursor_updates);
        let mut diagnostics = claude.diagnostics;
        diagnostics.extend(codex.diagnostics);
        Ok(UsageSnapshot { daily_usage, cursor_updates, diagnostics })
    }
}

#[derive(Debug, Clone)]
enum UsageRange {
    CurrentDay { since: String, until: String },
    FullHistory,
}

pub fn needs_full_history_poll(
    last_usage_poll_at: Option<OffsetDateTime>,
    now: OffsetDateTime,
) -> bool {
    let today_start = tokenmaxxing_day_start(tokenmaxxing_provider_day(now));
    last_usage_poll_at.is_none_or(|last_poll| last_poll < today_start)
}

impl AgentsviewDiscovery {
    pub fn discover() -> Self {
        let agentsview = std::env::var_os("GLORP_AGENTSVIEW_BIN")
            .map(PathBuf::from)
            .or_else(|| which::which("agentsview").ok());
        Self { agentsview }
    }

    pub fn from_sources<'a, E, P>(env: E, path: P) -> Result<Self>
    where
        E: IntoIterator<Item = (&'a str, &'a Path)>,
        P: IntoIterator<Item = &'a Path>,
    {
        let mut discovered = Self::default();
        for (name, value) in env {
            if name == "GLORP_AGENTSVIEW_BIN" {
                discovered.agentsview = Some(value.to_path_buf());
            }
        }
        for candidate in path {
            if discovered.agentsview.is_none() {
                discovered.agentsview = Some(candidate.to_path_buf());
            }
        }
        Ok(discovered)
    }
}

impl From<AgentsviewDiscovery> for AgentsviewPaths {
    fn from(value: AgentsviewDiscovery) -> Self {
        Self { agentsview: value.agentsview }
    }
}

fn result_from_parts(
    deltas: Vec<UsageDelta>,
    diagnostics: Vec<ProviderDiagnostic>,
) -> UsagePollResult {
    let total_tokens = deltas.iter().map(|delta| delta.total_tokens).sum();
    UsagePollResult {
        deltas,
        diagnostics,
        total_effective_tokens: total_tokens,
        total_tokens,
    }
}

fn empty_poll(diagnostics: Vec<ProviderDiagnostic>) -> UsagePollResult {
    UsagePollResult {
        deltas: Vec::new(),
        diagnostics,
        total_effective_tokens: 0.0,
        total_tokens: 0.0,
    }
}

#[allow(dead_code)]
fn snapshot_scope(agent: &str, requested_provider_days: Vec<Date>) -> ProviderSnapshotScope {
    ProviderSnapshotScope {
        collector_scope_id: format!("agentsview:{agent}:local-usage"),
        replacement_scope_id: format!("agentsview:{agent}:local-usage"),
        requested_provider_days,
    }
}

fn collector_surface(agent: &str) -> String {
    format!("agentsview:{agent}")
}

fn diagnostic(provider_surface: &str, code: &str, message: &str) -> ProviderDiagnostic {
    ProviderDiagnostic {
        provider_surface: provider_surface.to_string(),
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn filter_agentsview_json_to_requested_days(
    agent: &str,
    text: &str,
    requested_provider_days: &[Date],
) -> std::result::Result<(String, Vec<UnexpectedProviderDayDiagnostic>), ProviderDiagnostic> {
    let mut value: Value = serde_json::from_str(text).map_err(|_| {
        diagnostic(
            agent,
            "invalid_json",
            &format!("agentsview {agent} returned invalid_json"),
        )
    })?;
    let Some(rows) = value.get_mut("daily").and_then(Value::as_array_mut) else {
        return Ok((text.to_string(), Vec::new()));
    };

    let mut diagnostics = Vec::new();
    let mut kept = Vec::new();
    for row in std::mem::take(rows) {
        if let Some(provider_day) = provider_day_from_raw_agentsview_row(&row) {
            if !requested_provider_days.contains(&provider_day) {
                diagnostics.push(UnexpectedProviderDayDiagnostic {
                    diagnostic: diagnostic(
                        agent,
                        "unexpected_provider_day",
                        &format!("agentsview returned unrequested provider day {provider_day}"),
                    ),
                    provider_day,
                });
                continue;
            }
        }
        kept.push(row);
    }
    *rows = kept;
    Ok((value.to_string(), diagnostics))
}

fn provider_day_from_raw_agentsview_row(row: &Value) -> Option<Date> {
    row.get("date")
        .or_else(|| row.get("period"))
        .and_then(Value::as_str)
        .and_then(|period| parse_agentsview_period_date(period).ok())
        .map(|(_date, period_start)| period_start.date())
}

fn persist_diagnostic(store: &UsageStore, diagnostic: &ProviderDiagnostic) -> Result<()> {
    store.insert_diagnostic(&StoredProviderDiagnostic {
        provider_surface: diagnostic.provider_surface.clone(),
        code: diagnostic.code.clone(),
        message: diagnostic.message.clone(),
        recorded_at: OffsetDateTime::now_utc(),
    })
}

fn provider_diagnostic_from_stored(diagnostic: StoredProviderDiagnostic) -> ProviderDiagnostic {
    ProviderDiagnostic {
        provider_surface: diagnostic.provider_surface,
        code: diagnostic.code,
        message: diagnostic.message,
    }
}

fn write_prepared_snapshot(store: &mut UsageStore, prepared: &PreparedAgentSnapshot) -> Result<()> {
    let observed_at = OffsetDateTime::now_utc();
    let batch = ProviderSnapshotBatchInput {
        collector_scope_id: prepared.scope.collector_scope_id.clone(),
        collector_surface: prepared.collector_surface.clone(),
        command: prepared.command_name.clone(),
        token_contract: TOKENMAXXING_TOTAL_V1.to_string(),
        requested_provider_days: prepared.scope.requested_provider_days.clone(),
        covered_accounting_sources: prepared.covered_accounting_sources.clone(),
        provider_version: prepared.version.clone(),
        parser_version: prepared.version.clone(),
        observed_at,
    };
    store.write_provider_snapshot_batch(&batch, &prepared.rows, &[])?;
    Ok(())
}

fn record_snapshot_failures(
    store: &mut UsageStore,
    scope: &ProviderSnapshotScope,
    _agent: &str,
    diagnostics: &[ProviderDiagnostic],
    observed_at: OffsetDateTime,
) -> Result<()> {
    for diagnostic in diagnostics {
        if !should_record_snapshot_failure(diagnostic.code.as_str()) {
            continue;
        }
        for day in &scope.requested_provider_days {
            record_snapshot_diagnostic(
                store,
                scope,
                Some(scope.replacement_scope_id.clone()),
                Some(*day),
                "run_blocked",
                &diagnostic.code,
                &diagnostic.message,
                observed_at,
            )?;
        }
    }
    Ok(())
}

fn should_record_snapshot_failure(_code: &str) -> bool {
    true
}

#[allow(clippy::too_many_arguments)]
fn record_snapshot_diagnostic(
    store: &mut UsageStore,
    scope: &ProviderSnapshotScope,
    replacement_scope_id: Option<String>,
    provider_day: Option<Date>,
    diagnostic_kind: &str,
    reason_code: &str,
    message: &str,
    observed_at: OffsetDateTime,
) -> Result<()> {
    store.record_snapshot_failure(&ProviderSnapshotDiagnosticInput {
        diagnostic_kind: diagnostic_kind.to_string(),
        collector_scope_id: scope.collector_scope_id.clone(),
        replacement_scope_id,
        requested_provider_days: scope.requested_provider_days.clone(),
        provider_day,
        reason_code: reason_code.to_string(),
        message: message.to_string(),
        observed_at,
    })
}

fn blocks_requested_snapshot(code: &str) -> bool {
    matches!(
        code,
        "missing_token_fields" | "malformed_token_field" | "invalid_period_start"
    )
}

fn cursor_key_for_record(record: &NormalizedUsageRecord) -> Result<String> {
    let key = ProviderCursorKey {
        provider_surface: record.source_identity.provider_surface.clone(),
        token_contract: Some(TOKENMAXXING_TOTAL_V1.to_string()),
        command: AGENTSVIEW_COMMAND.to_string(),
        source_surface: "daily".to_string(),
        period_start: record.period_start.clone(),
        model: record.model.clone(),
        raw_source_id: Some(record.source_identity.display_name.clone()),
    };
    serde_json::to_string(&key).map_err(GlorpError::from)
}

fn cursor_update_for_record(
    record: &NormalizedUsageRecord,
    version: &str,
) -> Result<ProviderCursorUpdate> {
    Ok(ProviderCursorUpdate {
        provider_surface: record.source_identity.provider_surface.clone(),
        cursor_key: cursor_key_for_record(record)?,
        cursor_value: serde_json::to_string(&record.raw_totals)?,
        provider_version: version.to_string(),
        parser_version: version.to_string(),
    })
}

fn snapshot_row_for_record(
    scope: &ProviderSnapshotScope,
    agent: &str,
    version: &str,
    record: NormalizedUsageRecord,
    provider_day: Date,
) -> Result<ProviderSnapshotRowInput> {
    let cursor_key = cursor_key_for_record(&record)?;
    let cursor_update = ProviderCursorUpdate {
        provider_surface: record.source_identity.provider_surface.clone(),
        cursor_key: cursor_key.clone(),
        cursor_value: serde_json::to_string(&record.raw_totals)?,
        provider_version: version.to_string(),
        parser_version: version.to_string(),
    };
    Ok(ProviderSnapshotRowInput {
        replacement_scope_id: scope.replacement_scope_id.clone(),
        collector_scope_id: scope.collector_scope_id.clone(),
        collector_surface: collector_surface(agent),
        command: AGENTSVIEW_COMMAND.to_string(),
        token_contract: TOKENMAXXING_TOTAL_V1.to_string(),
        accounting_source: record.source_identity.provider_surface,
        provider_day,
        model: record.model,
        source_surface: "daily".to_string(),
        provider_period: record.period_start,
        raw_source_id_hash: Some(stable_hash(&record.source_identity.display_name)),
        cursor_key_hash: stable_hash(&cursor_key),
        cursor_update,
        raw_token_buckets: Some(record.raw_totals),
        total_tokens: record.raw_totals.total_tokens(),
        cost_usd: record.display_cost_usd,
        confidence: record.confidence,
    })
}

fn is_node_helper(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some("js") | Some("mjs")
    )
}

fn agentsview_helper_command(
    provider_surface: &str,
    helper: &Path,
    explicit_node: Option<&Path>,
) -> std::result::Result<HelperCommand, ProviderDiagnostic> {
    if is_node_helper(helper) {
        let node = explicit_node
            .map(Path::to_path_buf)
            .or_else(|| std::env::var_os("GLORP_NODE_BIN").map(PathBuf::from))
            .or_else(|| which::which("node").ok())
            .ok_or_else(|| {
                diagnostic(
                    provider_surface,
                    "missing_helper",
                    "agentsview helper requires node, but node was not found",
                )
            })?;
        if !node.exists() {
            return Err(diagnostic(
                provider_surface,
                "missing_helper",
                "agentsview helper requires node, but node was not found",
            ));
        }
        return Ok(HelperCommand {
            program: node,
            args_prefix: vec![helper.display().to_string()],
        });
    }

    Ok(HelperCommand {
        program: helper.to_path_buf(),
        args_prefix: Vec::new(),
    })
}

fn version_with_timeout(helper: &Path, timeout: Duration) -> Option<String> {
    let hcmd = agentsview_helper_command("agentsview", helper, None).ok()?;
    let mut command = Command::new(&hcmd.program);
    command.args(&hcmd.args_prefix);
    command.arg("--version");
    let output = run_command_with_timeout(&mut command, timeout).ok()?;
    if !output.status.success() {
        return None;
    }
    safe_version_line(&output.stdout)
}

fn safe_version_line(bytes: &[u8]) -> Option<String> {
    let raw_line = String::from_utf8_lossy(bytes)
        .lines()
        .next()?
        .trim()
        .to_string();
    let line = raw_line
        .split(" (")
        .next()
        .unwrap_or(raw_line.as_str())
        .trim()
        .to_string();
    if line.len() > 80
        || line.contains('{')
        || line.contains('}')
        || line.contains('"')
        || line.contains("prompt")
        || line.contains("response")
        || line.contains("tool")
    {
        return None;
    }
    if line
        .chars()
        .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '.' | '-' | '_' | '@')))
    {
        return None;
    }
    line.chars().any(|ch| ch.is_ascii_digit()).then_some(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    fn helper_path(dir: &tempfile::TempDir, stem: &str) -> PathBuf {
        #[cfg(windows)]
        {
            dir.path().join(format!("{stem}.cmd"))
        }
        #[cfg(not(windows))]
        {
            dir.path().join(stem)
        }
    }

    fn write_helper(path: &Path, unix_body: &str, windows_body: &str) {
        let mut file = std::fs::File::create(path).unwrap();
        if cfg!(windows) {
            writeln!(file, "{windows_body}").unwrap();
        } else {
            writeln!(file, "{unix_body}").unwrap();
        }
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[test]
    fn helper_command_node_helper_uses_node_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let mjs_helper = dir.path().join("agentsview.mjs");
        std::fs::write(&mjs_helper, "#!/usr/bin/env node\nconsole.log('{}');\n").unwrap();

        // Provide an explicit node binary so the test doesn't depend on PATH.
        let node_path = which::which("node").expect("node must be on PATH for this test to run");

        let cmd = agentsview_helper_command("claude", &mjs_helper, Some(&node_path))
            .expect("should build a valid HelperCommand");

        assert_eq!(cmd.program, node_path, "program should be the node binary");
        assert_eq!(
            cmd.args_prefix,
            vec![mjs_helper.display().to_string()],
            "args_prefix should contain the .mjs helper path"
        );
    }

    #[test]
    fn helper_command_native_helper_no_node_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let native_helper = helper_path(&dir, "agentsview-native");
        write_helper(
            &native_helper,
            "#!/usr/bin/env bash\nexit 0",
            "@echo off\nexit /b 0",
        );

        let cmd = agentsview_helper_command("claude", &native_helper, None)
            .expect("should build a valid HelperCommand");

        assert_eq!(
            cmd.program, native_helper,
            "program should be the helper itself"
        );
        assert!(
            cmd.args_prefix.is_empty(),
            "args_prefix should be empty for a native helper"
        );
    }

    #[test]
    fn version_probe_uses_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let helper = helper_path(&dir, "sleeping-version-helper");
        write_helper(
            &helper,
            "#!/usr/bin/env bash\nif [[ \"$1\" == \"--version\" ]]; then sleep 2; echo 'agentsview v0.32.1'; exit 0; fi\nexit 0",
            "@echo off\nif \"%~1\"==\"--version\" (\n  powershell -NoProfile -Command \"Start-Sleep -Seconds 2\" >NUL\n  echo agentsview v0.32.1\n  exit /b 0\n)\nexit /b 0",
        );

        let started = Instant::now();
        let version = version_with_timeout(&helper, Duration::from_millis(25));

        assert_eq!(version, None);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "version probe should time out quickly"
        );
    }

    #[test]
    fn usage_timeout_returns_helper_timeout_diagnostic() {
        let dir = tempfile::tempdir().unwrap();
        let helper = helper_path(&dir, "sleeping-usage-helper");
        write_helper(
            &helper,
            "#!/usr/bin/env bash\nif [[ \"$1\" == \"--version\" ]]; then echo 'agentsview v0.32.1'; exit 0; fi\nsleep 2\necho '{{\"daily\":[]}}'",
            "@echo off\nif \"%~1\"==\"--version\" (\n  echo agentsview v0.32.1\n  exit /b 0\n)\npowershell -NoProfile -Command \"Start-Sleep -Seconds 2\" >NUL\necho {\"daily\":[]}",
        );
        let provider = AgentsviewCommandProvider::new(AgentsviewPaths { agentsview: Some(helper) });
        let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();

        let started = Instant::now();
        let result = provider
            .poll_agent_with_timeout(
                &mut store,
                "claude",
                false,
                UsageRange::FullHistory,
                Duration::from_millis(25),
            )
            .unwrap();

        assert!(result.deltas.is_empty());
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].provider_surface, "claude");
        assert_eq!(result.diagnostics[0].code, "helper_timeout");
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "usage timeout path should not wait for the sleeping helper"
        );
        assert!(store
            .recent_diagnostics(5)
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic.provider_surface == "claude"
                && diagnostic.code == "helper_timeout"));
    }

    #[test]
    fn full_history_poll_is_only_needed_before_current_tokenmaxxing_day() {
        let current_day = time::macros::datetime!(2026 - 07 - 07 20:00 UTC);
        let before_la_midnight = time::macros::datetime!(2026 - 07 - 07 06:59 UTC);
        let after_la_midnight = time::macros::datetime!(2026 - 07 - 07 07:01 UTC);

        assert!(needs_full_history_poll(None, current_day));
        assert!(needs_full_history_poll(
            Some(before_la_midnight),
            current_day
        ));
        assert!(!needs_full_history_poll(
            Some(after_la_midnight),
            current_day
        ));
    }
}
