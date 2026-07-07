use crate::error::{GlorpError, Result};
use crate::storage::usage_store::{
    ProviderCursorUpdate, ProviderDiagnostic as StoredProviderDiagnostic, UsageStore,
};
use crate::usage::ccusage::{run_command_with_timeout, HelperCommand, HELPER_SUBPROCESS_TIMEOUT};
use crate::usage::day_axis::{
    parse_agentsview_period_date, tokenmaxxing_day_start, tokenmaxxing_provider_day,
    TOKENMAXXING_TIMEZONE,
};
use crate::usage::normalize::{normalize_agentsview_json, RawTokenTotals};
use crate::usage::provider::{
    ProviderCursorKey, ProviderDiagnostic, UsageDelta, UsagePollResult, UsageProvider,
    UsageSnapshot,
};
use crate::usage::token_contract::TOKENMAXXING_TOTAL_V1;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
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

    pub fn discovered_version(&self) -> Option<String> {
        let helper = self.paths.agentsview.as_deref()?;
        if !helper.exists() {
            return None;
        }
        self.version(helper)
    }

    pub fn poll_current_day(&self, store: &mut UsageStore) -> Result<UsagePollResult> {
        let now = OffsetDateTime::now_utc();
        let claude = self.poll_agent_current_day(store, "claude", false, now)?;
        let codex = self.poll_agent_current_day(store, "codex", true, now)?;

        let mut deltas = claude.deltas;
        deltas.extend(codex.deltas);
        let mut diagnostics = claude.diagnostics;
        diagnostics.extend(codex.diagnostics);
        Ok(result_from_parts(deltas, diagnostics))
    }

    pub fn poll_full_history(&self, store: &mut UsageStore) -> Result<UsagePollResult> {
        let claude = self.poll_agent_full_history(store, "claude", false)?;
        let codex = self.poll_agent_full_history(store, "codex", true)?;
        let mut deltas = claude.deltas;
        deltas.extend(codex.deltas);
        let mut diagnostics = claude.diagnostics;
        diagnostics.extend(codex.diagnostics);
        Ok(result_from_parts(deltas, diagnostics))
    }

    fn poll_agent_current_day(
        &self,
        store: &mut UsageStore,
        agent: &str,
        no_sync: bool,
        now: OffsetDateTime,
    ) -> Result<UsagePollResult> {
        let date = tokenmaxxing_provider_day(now).to_string();
        self.poll_agent_with_timeout(
            store,
            agent,
            no_sync,
            UsageRange::CurrentDay { since: date.clone(), until: date },
            HELPER_SUBPROCESS_TIMEOUT,
        )
    }

    fn poll_agent_full_history(
        &self,
        store: &mut UsageStore,
        agent: &str,
        no_sync: bool,
    ) -> Result<UsagePollResult> {
        self.poll_agent_with_timeout(
            store,
            agent,
            no_sync,
            UsageRange::FullHistory,
            HELPER_SUBPROCESS_TIMEOUT,
        )
    }

    fn poll_agent_with_timeout(
        &self,
        store: &mut UsageStore,
        agent: &str,
        no_sync: bool,
        range: UsageRange,
        timeout: Duration,
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
                return Ok(empty_poll(vec![diagnostic]));
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
                token_contract: Some(TOKENMAXXING_TOTAL_V1.to_string()),
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

    fn snapshot_for_calibration(&self, store: &mut UsageStore) -> Result<UsageSnapshot> {
        let poll = self.poll_full_history(store)?;
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
