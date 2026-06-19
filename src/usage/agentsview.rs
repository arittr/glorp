use crate::error::{GlorpError, Result};
use crate::storage::usage_store::{
    ProviderCursorUpdate, ProviderDiagnostic as StoredProviderDiagnostic, UsageStore,
};
use crate::usage::ccusage::{run_command_with_timeout, HELPER_SUBPROCESS_TIMEOUT};
use crate::usage::day_axis::{parse_agentsview_period_date, TOKENMAXXING_TIMEZONE};
use crate::usage::normalize::{normalize_agentsview_json, RawTokenTotals};
use crate::usage::provider::{
    ProviderCursorKey, ProviderDiagnostic, UsageDelta, UsagePollResult, UsageProvider,
    UsageSnapshot,
};
use crate::usage::token_contract::TOKENMAXXING_TOTAL_V1;
use std::path::{Path, PathBuf};
use std::process::Command;
use time::OffsetDateTime;

pub const AGENTSVIEW_COMMAND: &str = "agentsview usage daily";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentsviewPaths {
    pub agentsview: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentsviewDiscovery {
    pub agentsview: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct AgentsviewCommandProvider {
    paths: AgentsviewPaths,
}

impl AgentsviewCommandProvider {
    pub fn new(paths: AgentsviewPaths) -> Self {
        Self { paths }
    }

    pub fn from_environment() -> Self {
        Self::new(AgentsviewDiscovery::discover().into())
    }

    fn poll_agent(
        &self,
        store: &mut UsageStore,
        agent: &str,
        no_sync: bool,
    ) -> Result<UsagePollResult> {
        let Some(helper) = self.paths.agentsview.as_deref() else {
            let diagnostic = diagnostic(agent, "missing_helper", "agentsview helper was not found");
            persist_diagnostic(store, &diagnostic)?;
            return Ok(empty_poll(vec![diagnostic]));
        };
        if !helper.exists() {
            let diagnostic = diagnostic(agent, "missing_helper", "agentsview helper was not found");
            persist_diagnostic(store, &diagnostic)?;
            return Ok(empty_poll(vec![diagnostic]));
        }

        let version = self
            .version(helper)
            .unwrap_or_else(|| "unknown".to_string());
        let output = self.run_usage(helper, agent, no_sync)?;
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let diagnostic = diagnostic(
                agent,
                "helper_exit",
                &format!("agentsview exited with status {code}"),
            );
            persist_diagnostic(store, &diagnostic)?;
            return Ok(empty_poll(vec![diagnostic]));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let batch = match normalize_agentsview_json(agent, &stdout) {
            Ok(batch) => batch,
            Err(diagnostic) => {
                persist_diagnostic(store, &diagnostic)?;
                return Ok(empty_poll(vec![diagnostic]));
            }
        };
        for diagnostic in &batch.diagnostics {
            persist_diagnostic(store, diagnostic)?;
        }

        let mut deltas = Vec::new();
        let mut diagnostics = Vec::new();
        let observed_at = OffsetDateTime::now_utc();
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

            let key = ProviderCursorKey {
                provider_surface: record.source_identity.provider_surface.clone(),
                command: AGENTSVIEW_COMMAND.to_string(),
                source_surface: "daily".to_string(),
                period_start: record.period_start.clone(),
                model: record.model.clone(),
                raw_source_id: Some(record.source_identity.display_name.clone()),
            };
            let cursor_key = serde_json::to_string(&key)?;
            let cursor_partition = record.source_identity.provider_surface.clone();
            let previous_raw = match store.provider_cursor(&cursor_partition, &cursor_key) {
                Ok(value) => value,
                Err(_) => {
                    let diagnostic =
                        diagnostic(agent, "cursor_corruption", "agentsview cursor_corruption");
                    persist_diagnostic(store, &diagnostic)?;
                    diagnostics.push(diagnostic);
                    None
                }
            };
            let previous = match previous_raw {
                Some(value) => match serde_json::from_str::<RawTokenTotals>(&value) {
                    Ok(parsed) => parsed,
                    Err(_) => {
                        let diagnostic =
                            diagnostic(agent, "cursor_corruption", "agentsview cursor_corruption");
                        persist_diagnostic(store, &diagnostic)?;
                        diagnostics.push(diagnostic);
                        RawTokenTotals::default()
                    }
                },
                None => RawTokenTotals::default(),
            };

            let Some(delta_totals) = record.raw_totals.positive_delta_since(previous) else {
                let diagnostic = diagnostic(
                    agent,
                    "cursor_total_decreased",
                    "agentsview cursor_total_decreased",
                );
                persist_diagnostic(store, &diagnostic)?;
                diagnostics.push(diagnostic);
                write_cursor(
                    store,
                    &cursor_partition,
                    &cursor_key,
                    record.raw_totals,
                    &version,
                )?;
                continue;
            };
            if !delta_totals.has_positive_effective_bucket() {
                continue;
            }

            let total_tokens = delta_totals.total_tokens();
            let cursor_value = serde_json::to_string(&record.raw_totals)?;
            deltas.push(UsageDelta {
                provider_surface: record.source_identity.provider_surface.clone(),
                source_identity: record.source_identity,
                command: AGENTSVIEW_COMMAND.to_string(),
                effective_tokens: total_tokens,
                total_tokens,
                token_contract: TOKENMAXXING_TOTAL_V1.to_string(),
                confidence: record.confidence,
                period_start,
                observed_at,
                model: record.model,
                cursor_update: ProviderCursorUpdate {
                    provider_surface: cursor_partition,
                    cursor_key,
                    cursor_value,
                    provider_version: version.clone(),
                    parser_version: version.clone(),
                },
                token_totals: Some(delta_totals),
            });
        }

        diagnostics.extend(batch.diagnostics);
        Ok(result_from_parts(deltas, diagnostics))
    }

    fn version(&self, helper: &Path) -> Option<String> {
        let output = Command::new(helper).arg("--version").output().ok()?;
        if !output.status.success() {
            return None;
        }
        safe_version_line(&output.stdout)
    }

    fn run_usage(&self, helper: &Path, agent: &str, no_sync: bool) -> Result<std::process::Output> {
        let mut command = Command::new(helper);
        command.args([
            "usage",
            "daily",
            "--json",
            "--breakdown",
            "--agent",
            agent,
            "--since",
            "1970-01-01",
            "--timezone",
            TOKENMAXXING_TIMEZONE,
        ]);
        if no_sync {
            command.arg("--no-sync");
        }
        match run_command_with_timeout(&mut command, HELPER_SUBPROCESS_TIMEOUT) {
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
        let claude = self.poll_agent(store, "claude", false)?;
        let codex = self.poll_agent(store, "codex", true)?;

        let mut deltas = claude.deltas;
        deltas.extend(codex.deltas);
        let mut diagnostics = claude.diagnostics;
        diagnostics.extend(codex.diagnostics);
        Ok(result_from_parts(deltas, diagnostics))
    }

    fn snapshot_for_calibration(&self, store: &mut UsageStore) -> Result<UsageSnapshot> {
        let poll = self.poll(store)?;
        let daily_usage = poll
            .deltas
            .iter()
            .map(|delta| {
                crate::game::calibration::DailyUsage::with_activity_timestamp(
                    delta.period_start,
                    delta.total_tokens,
                )
            })
            .collect();
        let cursor_updates = poll
            .deltas
            .iter()
            .map(|delta| delta.cursor_update.clone())
            .collect();
        Ok(UsageSnapshot {
            daily_usage,
            cursor_updates,
            diagnostics: poll.diagnostics,
        })
    }
}

impl AgentsviewDiscovery {
    pub fn discover() -> Self {
        let agentsview = std::env::var_os("GLORP_AGENTSVIEW_BIN")
            .map(PathBuf::from)
            .or_else(|| which::which("agentsview").ok());
        Self { agentsview }
    }
}

impl From<AgentsviewDiscovery> for AgentsviewPaths {
    fn from(value: AgentsviewDiscovery) -> Self {
        Self {
            agentsview: value.agentsview,
        }
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

fn diagnostic(provider_surface: &str, code: &str, message: &str) -> ProviderDiagnostic {
    ProviderDiagnostic {
        provider_surface: provider_surface.to_string(),
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn persist_diagnostic(store: &UsageStore, diagnostic: &ProviderDiagnostic) -> Result<()> {
    store.insert_diagnostic(&StoredProviderDiagnostic {
        provider_surface: diagnostic.provider_surface.clone(),
        code: diagnostic.code.clone(),
        message: diagnostic.message.clone(),
        recorded_at: OffsetDateTime::now_utc(),
    })
}

fn write_cursor(
    store: &mut UsageStore,
    provider_surface: &str,
    cursor_key: &str,
    raw_totals: RawTokenTotals,
    version: &str,
) -> Result<()> {
    store.set_provider_cursor(
        provider_surface,
        cursor_key,
        &serde_json::to_string(&raw_totals)?,
        version,
        version,
    )
}

fn safe_version_line(bytes: &[u8]) -> Option<String> {
    let line = String::from_utf8_lossy(bytes)
        .lines()
        .next()?
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
        .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '.' | '-' | '_' | '/' | '@')))
    {
        return None;
    }
    line.chars().any(|ch| ch.is_ascii_digit()).then_some(line)
}
