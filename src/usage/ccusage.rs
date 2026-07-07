use crate::error::{GlorpError, Result};
use crate::game::effective_tokens::EffectiveTokenWeights;
use crate::storage::usage_store::{
    ProviderCursorUpdate, ProviderDiagnostic as StoredProviderDiagnostic, UsageStore,
};
use crate::usage::normalize::{normalize_usage_json, NormalizedUsageRecord, RawTokenTotals};
use crate::usage::provider::{
    ProviderCursorKey, ProviderDiagnostic, UsageDelta, UsagePollResult, UsageProvider,
    UsageSnapshot,
};
use crate::usage::token_contract::TOKENMAXXING_TOTAL_V1;
use std::ffi::OsStr;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Duration;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};
use wait_timeout::ChildExt;

const CLAUDE_SURFACE: &str = "claude-code";
const CODEX_SURFACE: &str = "codex";
const CONFIDENCE: &str = "local-log-derived";

/// Hard ceiling for a single ccusage helper invocation. A helper that hangs
/// past this (frozen Node startup, slow fs lock, runaway loop) gets killed
/// so the watch worker thread cannot wedge.
pub(crate) const HELPER_SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperCommand {
    pub program: PathBuf,
    pub args_prefix: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HelperPaths {
    pub unified: Option<PathBuf>,
    pub claude: Option<PathBuf>,
    pub codex: Option<PathBuf>,
    pub node: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HelperDiscovery {
    pub claude: Option<PathBuf>,
    pub codex: Option<PathBuf>,
    pub node: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct CcusageCommandProvider {
    helpers: HelperPaths,
}

enum HelperInvocation {
    Records {
        version: String,
        records: Vec<NormalizedUsageRecord>,
        diagnostics: Vec<ProviderDiagnostic>,
    },
    EarlyExit {
        diagnostics: Vec<ProviderDiagnostic>,
    },
}

impl CcusageCommandProvider {
    pub fn new(helpers: HelperPaths) -> Self {
        Self { helpers }
    }

    pub fn with_weights(self, weights: EffectiveTokenWeights) -> Self {
        let _ = weights;
        self
    }

    pub fn from_environment() -> Self {
        Self::new(HelperDiscovery::discover().into())
    }

    pub fn from_environment_with_weights(weights: EffectiveTokenWeights) -> Self {
        Self::new(HelperDiscovery::discover().into()).with_weights(weights)
    }

    fn invoke_helper(
        &self,
        store: &mut UsageStore,
        provider_surface: &str,
        command_name: &str,
        helper: Option<&Path>,
        daily_args: &[&str],
    ) -> Result<HelperInvocation> {
        let Some(helper) = helper else {
            let diagnostic = diagnostic(
                provider_surface,
                "missing_helper",
                &format!("{command_name} helper was not found"),
            );
            persist_diagnostic(store, &diagnostic)?;
            return Ok(HelperInvocation::EarlyExit { diagnostics: vec![diagnostic] });
        };

        let helper_command = match helper_command(
            provider_surface,
            command_name,
            helper,
            self.helpers.node.as_deref(),
        ) {
            Ok(helper_command) => helper_command,
            Err(diagnostic) => {
                persist_diagnostic(store, &diagnostic)?;
                return Ok(HelperInvocation::EarlyExit { diagnostics: vec![diagnostic] });
            }
        };
        let version = self
            .run_command(provider_surface, &helper_command, &["--version"])
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    safe_version_line(&output.stdout)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "unknown".to_string());

        // ccusage >= 20 turned the bare `daily` command into an all-agents
        // aggregator (gpt/gemini/codex rows included) and renamed `date` to
        // `period`. The `claude daily` subcommand keeps the claude-only
        // legacy shape; older helpers don't know the subcommand, so gate on
        // the probed version. Non-claude surfaces are untouched.
        let scoped_args: Vec<&str>;
        let daily_args: &[&str] = if provider_surface == CLAUDE_SURFACE
            && ccusage_major_version(&version).is_some_and(|major| major >= 20)
        {
            scoped_args = std::iter::once("claude")
                .chain(daily_args.iter().copied())
                .collect();
            &scoped_args
        } else {
            daily_args
        };

        let output = match self.run_command(provider_surface, &helper_command, daily_args) {
            Ok(output) => output,
            Err(GlorpError::Io(err)) if err.kind() == io::ErrorKind::TimedOut => {
                let diagnostic = diagnostic(
                    provider_surface,
                    "helper_timeout",
                    &format!(
                        "{command_name} did not return within {}s",
                        HELPER_SUBPROCESS_TIMEOUT.as_secs()
                    ),
                );
                persist_diagnostic(store, &diagnostic)?;
                return Ok(HelperInvocation::EarlyExit { diagnostics: vec![diagnostic] });
            }
            Err(err) => return Err(err),
        };
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let diagnostic = diagnostic(
                provider_surface,
                "helper_exit",
                &format!("{command_name} exited with status {code}"),
            );
            persist_diagnostic(store, &diagnostic)?;
            return Ok(HelperInvocation::EarlyExit { diagnostics: vec![diagnostic] });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let batch = match normalize_usage_json(provider_surface, &stdout) {
            Ok(batch) => batch,
            Err(diagnostic) => {
                persist_diagnostic(store, &diagnostic)?;
                return Ok(HelperInvocation::EarlyExit { diagnostics: vec![diagnostic] });
            }
        };
        for diagnostic in &batch.diagnostics {
            persist_diagnostic(store, diagnostic)?;
        }
        let records = batch.records;

        Ok(HelperInvocation::Records {
            version,
            records,
            diagnostics: batch.diagnostics,
        })
    }

    fn poll_helper(
        &self,
        store: &mut UsageStore,
        provider_surface: &str,
        command_name: &str,
        helper: Option<&Path>,
        daily_args: &[&str],
    ) -> Result<UsagePollResult> {
        let (version, records, invoke_diagnostics) =
            match self.invoke_helper(store, provider_surface, command_name, helper, daily_args)? {
                HelperInvocation::Records { version, records, diagnostics } => {
                    (version, records, diagnostics)
                }
                HelperInvocation::EarlyExit { diagnostics } => {
                    return Ok(UsagePollResult {
                        deltas: Vec::new(),
                        diagnostics,
                        total_effective_tokens: 0.0,
                        total_tokens: 0.0,
                    });
                }
            };

        // Record the helper version on the metadata cursor. This is a sentinel row
        // distinct from the data cursors that gate `UsageDelta` emission: data
        // cursors use a JSON-serialized `ProviderCursorKey` and only advance after
        // the unapplied ledger row is applied and pet state is saved. The metadata
        // cursor exists so `glorp doctor` can report the running helper version
        // even when no apply has happened yet.
        let metadata_key = helper_version_metadata_key(provider_surface, command_name);
        store.set_provider_cursor(provider_surface, &metadata_key, "{}", &version, &version)?;

        let mut deltas = Vec::new();
        let mut diagnostics = Vec::new();
        let observed_at = OffsetDateTime::now_utc();
        for record in records {
            let parsed_period_start = match parse_period_start(&record.period_start) {
                Ok(parsed) => parsed,
                Err(_) => {
                    let diagnostic = diagnostic(
                        provider_surface,
                        "invalid_period_start",
                        &format!(
                            "{provider_surface} invalid_period_start for period {} model {}",
                            record.period_start,
                            record.model.as_deref().unwrap_or("none")
                        ),
                    );
                    persist_diagnostic(store, &diagnostic)?;
                    diagnostics.push(diagnostic);
                    continue;
                }
            };

            let key = provider_cursor_key_for_record(&record, command_name);
            let cursor_key = cursor_key(&key)?;
            let cursor_partition = record.source_identity.provider_surface.clone();
            let previous_raw = match store.provider_cursor(&cursor_partition, &cursor_key) {
                Ok(Some(value)) => Some(value),
                Ok(None) => match read_legacy_cursor_value(
                    store,
                    &cursor_partition,
                    command_name,
                    &record.period_start,
                    record.model.clone(),
                    &version,
                ) {
                    Ok(Some(value)) => {
                        store.set_provider_cursor(
                            &cursor_partition,
                            &cursor_key,
                            &value,
                            &version,
                            &version,
                        )?;
                        Some(value)
                    }
                    Ok(None) => None,
                    Err(_) => {
                        let diagnostic = diagnostic(
                            provider_surface,
                            "cursor_corruption",
                            &format!("{provider_surface} cursor_corruption for {cursor_key}"),
                        );
                        persist_diagnostic(store, &diagnostic)?;
                        diagnostics.push(diagnostic);
                        None
                    }
                },
                Err(_) => {
                    let diagnostic = diagnostic(
                        provider_surface,
                        "cursor_corruption",
                        &format!("{provider_surface} cursor_corruption for {cursor_key}"),
                    );
                    persist_diagnostic(store, &diagnostic)?;
                    diagnostics.push(diagnostic);
                    None
                }
            };

            let previous = match previous_raw {
                Some(value) => match serde_json::from_str::<RawTokenTotals>(&value) {
                    Ok(parsed) => parsed,
                    Err(_) => {
                        let diagnostic = diagnostic(
                            provider_surface,
                            "cursor_corruption",
                            &format!("{provider_surface} cursor_corruption for {cursor_key}"),
                        );
                        persist_diagnostic(store, &diagnostic)?;
                        diagnostics.push(diagnostic);
                        RawTokenTotals::default()
                    }
                },
                None => RawTokenTotals::default(),
            };

            let Some(delta_totals) = record.raw_totals.positive_delta_since(previous) else {
                let diagnostic = diagnostic(
                    provider_surface,
                    "cursor_total_decreased",
                    &format!("{provider_surface} cursor_total_decreased for {cursor_key}"),
                );
                persist_diagnostic(store, &diagnostic)?;
                diagnostics.push(diagnostic);
                // cursor_total_decreased is a hard reset (not a normal advance).
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
                // Empty delta: nothing to feed the pet and nothing to stage. The cursor
                // stays put; the next poll will recompute the same empty delta and skip
                // again. No double-counting risk because no UsageDelta is emitted.
                continue;
            }

            let total_tokens = delta_totals.total_tokens();
            let cursor_value = serde_json::to_string(&record.raw_totals)?;
            let cursor_update = ProviderCursorUpdate {
                provider_surface: cursor_partition,
                cursor_key: cursor_key.clone(),
                cursor_value,
                provider_version: version.clone(),
                parser_version: version.clone(),
            };
            // The cursor advance is deferred to `mark_events_applied_and_advance_cursors`,
            // which runs after pet state is saved. This guarantees the unapplied ledger
            // row is durable before we forget the source totals; a save failure leaves
            // both the row and the unchanged cursor so the next successful run recovers
            // via `apply_unapplied_usage`.
            deltas.push(UsageDelta {
                provider_surface: record.source_identity.provider_surface.clone(),
                source_identity: record.source_identity.clone(),
                command: command_name.to_string(),
                effective_tokens: total_tokens,
                total_tokens,
                token_contract: TOKENMAXXING_TOTAL_V1.to_string(),
                confidence: CONFIDENCE.to_string(),
                period_start: parsed_period_start,
                observed_at,
                model: record.model,
                cursor_update,
                token_totals: Some(delta_totals),
                feed_highwater_updates: Vec::new(),
            });
        }

        let total_effective_tokens = deltas.iter().map(|delta| delta.effective_tokens).sum();
        diagnostics.extend(invoke_diagnostics);
        Ok(UsagePollResult {
            deltas,
            diagnostics,
            total_effective_tokens,
            total_tokens: total_effective_tokens,
        })
    }

    fn snapshot_helper(
        &self,
        store: &mut UsageStore,
        provider_surface: &str,
        command_name: &str,
        helper: Option<&Path>,
        daily_args: &[&str],
    ) -> Result<UsageSnapshot> {
        let (version, records, invoke_diagnostics) =
            match self.invoke_helper(store, provider_surface, command_name, helper, daily_args)? {
                HelperInvocation::Records { version, records, diagnostics } => {
                    (version, records, diagnostics)
                }
                HelperInvocation::EarlyExit { diagnostics } => {
                    return Ok(UsageSnapshot {
                        daily_usage: Vec::new(),
                        cursor_updates: Vec::new(),
                        diagnostics,
                    });
                }
            };

        let mut daily_usage = Vec::new();
        let mut cursor_updates = Vec::new();
        let mut diagnostics = Vec::new();
        for record in records {
            let parsed_period_start = match parse_period_start(&record.period_start) {
                Ok(parsed) => parsed,
                Err(_) => {
                    let diagnostic = diagnostic(
                        provider_surface,
                        "invalid_period_start",
                        &format!(
                            "{provider_surface} invalid_period_start for period {} model {}",
                            record.period_start,
                            record.model.as_deref().unwrap_or("none")
                        ),
                    );
                    persist_diagnostic(store, &diagnostic)?;
                    diagnostics.push(diagnostic);
                    continue;
                }
            };

            daily_usage.push(
                crate::game::calibration::DailyUsage::with_activity_timestamp(
                    parsed_period_start,
                    record.raw_totals.total_tokens(),
                ),
            );

            let key = provider_cursor_key_for_record(&record, command_name);
            let cursor_key = cursor_key(&key)?;
            let cursor_value = serde_json::to_string(&record.raw_totals)?;
            cursor_updates.push(ProviderCursorUpdate {
                provider_surface: record.source_identity.provider_surface.clone(),
                cursor_key,
                cursor_value,
                provider_version: version.clone(),
                parser_version: version.clone(),
            });
        }

        diagnostics.extend(invoke_diagnostics);
        Ok(UsageSnapshot { daily_usage, cursor_updates, diagnostics })
    }

    fn run_command(
        &self,
        _provider_surface: &str,
        helper: &HelperCommand,
        args: &[&str],
    ) -> Result<std::process::Output> {
        let mut command = Command::new(&helper.program);
        command.args(&helper.args_prefix);
        command.args(args);
        run_command_with_timeout(&mut command, HELPER_SUBPROCESS_TIMEOUT)
    }
}

/// Spawn `command` and wait up to `timeout` for it to finish. On timeout the
/// child is killed and a `TimedOut` IO error is returned, which the caller
/// surfaces as a `helper_timeout` diagnostic. Stdout / stderr are captured
/// the same way `Command::output` would.
pub(crate) fn run_command_with_timeout(command: &mut Command, timeout: Duration) -> Result<Output> {
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.stdin(Stdio::null());
    let mut child = command.spawn().map_err(GlorpError::from)?;
    let stdout_handle = child.stdout.take().map(|mut handle| {
        std::thread::spawn(move || {
            let mut stdout = Vec::new();
            let _ = handle.read_to_end(&mut stdout);
            stdout
        })
    });
    let stderr_handle = child.stderr.take().map(|mut handle| {
        std::thread::spawn(move || {
            let mut stderr = Vec::new();
            let _ = handle.read_to_end(&mut stderr);
            stderr
        })
    });

    let status = match child.wait_timeout(timeout).map_err(GlorpError::from)? {
        Some(status) => status,
        None => {
            // Best-effort kill: even if the kill itself fails the child will be
            // reaped when its parent (the watch worker thread or the test) exits.
            let _ = child.kill();
            let _ = child.wait();
            return Err(GlorpError::from(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("helper subprocess exceeded {}s timeout", timeout.as_secs()),
            )));
        }
    };

    let stdout = stdout_handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    let stderr = stderr_handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();

    Ok(Output { status, stdout, stderr })
}

impl UsageProvider for CcusageCommandProvider {
    fn poll(&self, store: &mut UsageStore) -> Result<UsagePollResult> {
        let unified = self.poll_helper(
            store,
            "unified",
            "ccusage",
            self.helpers.unified.as_deref(),
            &["daily", "--json", "--offline", "--order", "asc"],
        )?;
        // Unified is the preferred path. Only fall back to the legacy per-surface
        // helpers when it reports no effective usage.
        if unified.total_effective_tokens > 0.0 {
            return Ok(unified);
        }

        let claude = self.poll_helper(
            store,
            CLAUDE_SURFACE,
            "ccusage",
            self.helpers.claude.as_deref(),
            &["daily", "--json", "--offline", "--order", "asc"],
        )?;
        let codex = self.poll_helper(
            store,
            CODEX_SURFACE,
            "ccusage-codex",
            self.helpers.codex.as_deref(),
            &["daily", "--json", "--offline"],
        )?;

        let mut deltas = claude.deltas;
        deltas.extend(codex.deltas);
        let total_effective_tokens = deltas.iter().map(|delta| delta.effective_tokens).sum();
        let focused_diagnostics = claude
            .diagnostics
            .iter()
            .chain(codex.diagnostics.iter())
            .cloned()
            .collect::<Vec<_>>();
        let mut diagnostics = if focused_diagnostics.is_empty() {
            Vec::new()
        } else {
            unified.diagnostics
        };
        diagnostics.extend(focused_diagnostics);
        Ok(UsagePollResult {
            deltas,
            diagnostics,
            total_effective_tokens,
            total_tokens: total_effective_tokens,
        })
    }

    fn snapshot_for_calibration(&self, store: &mut UsageStore) -> Result<UsageSnapshot> {
        let unified = self.snapshot_helper(
            store,
            "unified",
            "ccusage",
            self.helpers.unified.as_deref(),
            &["daily", "--json", "--offline", "--order", "asc"],
        )?;
        // Unified is the preferred path. Fall back to legacy helpers when it
        // produced no daily usage or cursor updates.
        if !unified.daily_usage.is_empty() || !unified.cursor_updates.is_empty() {
            return Ok(unified);
        }

        let claude = self.snapshot_helper(
            store,
            CLAUDE_SURFACE,
            "ccusage",
            self.helpers.claude.as_deref(),
            &["daily", "--json", "--offline", "--order", "asc"],
        )?;
        let codex = self.snapshot_helper(
            store,
            CODEX_SURFACE,
            "ccusage-codex",
            self.helpers.codex.as_deref(),
            &["daily", "--json", "--offline"],
        )?;

        let mut daily_usage = claude.daily_usage;
        daily_usage.extend(codex.daily_usage);
        let mut cursor_updates = claude.cursor_updates;
        cursor_updates.extend(codex.cursor_updates);
        let focused_diagnostics = claude
            .diagnostics
            .iter()
            .chain(codex.diagnostics.iter())
            .cloned()
            .collect::<Vec<_>>();
        let mut diagnostics = if focused_diagnostics.is_empty() {
            Vec::new()
        } else {
            unified.diagnostics
        };
        diagnostics.extend(focused_diagnostics);
        Ok(UsageSnapshot { daily_usage, cursor_updates, diagnostics })
    }
}

impl HelperDiscovery {
    pub fn discover() -> Self {
        let claude = std::env::var_os("GLORP_CCUSAGE_BIN")
            .map(PathBuf::from)
            .or_else(|| find_on_path("ccusage"));
        let codex = std::env::var_os("GLORP_CCUSAGE_CODEX_BIN")
            .map(PathBuf::from)
            .or_else(|| find_on_path("ccusage-codex"));
        let node = std::env::var_os("GLORP_NODE_BIN")
            .map(PathBuf::from)
            .or_else(|| find_on_path("node"));
        let mut discovered = Self { claude, codex, node };
        if discovered.claude.is_none() || discovered.codex.is_none() || discovered.node.is_none() {
            if let Ok(paths) = crate::paths::AppPaths::resolve() {
                let locator_path = paths
                    .config_dir
                    .join(crate::usage::helper_locator::HELPER_LOCATOR_FILE);
                if let Ok(Some(locator)) =
                    crate::usage::helper_locator::read_helper_locator(&locator_path)
                {
                    if discovered.claude.is_none() {
                        discovered.claude = locator.ccusage_bin;
                    }
                    if discovered.codex.is_none() {
                        discovered.codex = locator.ccusage_codex_bin;
                    }
                    if discovered.node.is_none() {
                        discovered.node = locator.node_bin;
                    }
                }
            }
        }
        discovered
    }

    pub fn from_sources<'a, E, P>(env: E, path: P) -> Result<Self>
    where
        E: IntoIterator<Item = (&'a str, &'a Path)>,
        P: IntoIterator<Item = &'a Path>,
    {
        let mut discovered = HelperDiscovery::default();
        for (name, value) in env {
            match name {
                "GLORP_CCUSAGE_BIN" => discovered.claude = Some(value.to_path_buf()),
                "GLORP_CCUSAGE_CODEX_BIN" => discovered.codex = Some(value.to_path_buf()),
                "GLORP_NODE_BIN" => discovered.node = Some(value.to_path_buf()),
                _ => {}
            }
        }

        for candidate in path {
            if discovered.claude.is_none() {
                discovered.claude = Some(candidate.to_path_buf());
            } else if discovered.codex.is_none() {
                discovered.codex = Some(candidate.to_path_buf());
            }
        }

        Ok(discovered)
    }
}

impl From<HelperDiscovery> for HelperPaths {
    fn from(discovery: HelperDiscovery) -> Self {
        Self {
            unified: discovery.claude.clone(),
            claude: discovery.claude,
            codex: discovery.codex,
            node: discovery.node,
        }
    }
}

fn helper_command(
    provider_surface: &str,
    command_name: &str,
    helper: &Path,
    explicit_node: Option<&Path>,
) -> std::result::Result<HelperCommand, ProviderDiagnostic> {
    if !helper.exists() {
        return Err(diagnostic(
            provider_surface,
            "missing_helper",
            &format!("{command_name} helper was not found"),
        ));
    }

    if is_node_helper(helper) {
        let node = explicit_node
            .map(Path::to_path_buf)
            .or_else(|| std::env::var_os("GLORP_NODE_BIN").map(PathBuf::from))
            .or_else(|| find_on_path("node"))
            .ok_or_else(|| {
                diagnostic(
                    provider_surface,
                    "missing_helper",
                    &format!("{command_name} helper requires node, but node was not found"),
                )
            })?;
        if !node.exists() {
            return Err(diagnostic(
                provider_surface,
                "missing_helper",
                &format!("{command_name} helper requires node, but node was not found"),
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

fn is_node_helper(path: &Path) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some("js") | Some("mjs")
    )
}

fn find_on_path(command: &str) -> Option<PathBuf> {
    which::which(command).ok()
}

/// Major version from a `--version` line like "ccusage 20.0.6" (tolerates a
/// leading binary name and a `v` prefix). None when unparseable, which keeps
/// unknown helpers on the legacy invocation.
fn ccusage_major_version(version_line: &str) -> Option<u32> {
    let token = version_line.split_whitespace().last()?;
    token
        .trim_start_matches('v')
        .split('.')
        .next()?
        .parse()
        .ok()
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

fn diagnostic(provider_surface: &str, code: &str, message: &str) -> ProviderDiagnostic {
    ProviderDiagnostic {
        provider_surface: provider_surface.to_string(),
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn persist_diagnostic(store: &mut UsageStore, diagnostic: &ProviderDiagnostic) -> Result<()> {
    store.insert_diagnostic(&StoredProviderDiagnostic {
        provider_surface: diagnostic.provider_surface.clone(),
        code: diagnostic.code.clone(),
        message: diagnostic.message.clone(),
        recorded_at: OffsetDateTime::now_utc(),
    })
}

fn cursor_key(key: &ProviderCursorKey) -> Result<String> {
    serde_json::to_string(key).map_err(GlorpError::from)
}

fn provider_cursor_key_for_record(
    record: &NormalizedUsageRecord,
    command_name: &str,
) -> ProviderCursorKey {
    ProviderCursorKey {
        provider_surface: record.source_identity.provider_surface.clone(),
        token_contract: Some(TOKENMAXXING_TOTAL_V1.to_string()),
        command: command_name.to_string(),
        source_surface: "daily".to_string(),
        period_start: record.period_start.clone(),
        model: record.model.clone(),
        raw_source_id: None,
    }
}

// Sentinel cursor key used to record the helper version without claiming any
// real cursor position. The "::" delimiter and lack of JSON braces guarantee
// this string never collides with a real `ProviderCursorKey` JSON.
fn helper_version_metadata_key(provider_surface: &str, command_name: &str) -> String {
    format!("helper_version::{provider_surface}::{command_name}")
}

fn legacy_cursor_key_with_parser_version(
    provider_surface: &str,
    command: &str,
    parser_version: &str,
    period_start: &str,
    model: Option<String>,
) -> Result<String> {
    #[derive(serde::Serialize)]
    struct LegacyKey {
        provider_surface: String,
        command: String,
        parser_version: String,
        period_start: String,
        model: Option<String>,
    }
    serde_json::to_string(&LegacyKey {
        provider_surface: provider_surface.to_string(),
        command: command.to_string(),
        parser_version: parser_version.to_string(),
        period_start: period_start.to_string(),
        model,
    })
    .map_err(GlorpError::from)
}

fn read_legacy_cursor_value(
    store: &UsageStore,
    provider_surface: &str,
    command: &str,
    period_start: &str,
    model: Option<String>,
    version: &str,
) -> Result<Option<String>> {
    let legacy_key = legacy_cursor_key_with_parser_version(
        provider_surface,
        command,
        version,
        period_start,
        model,
    )?;
    store.provider_cursor(provider_surface, &legacy_key)
}

fn write_cursor(
    store: &mut UsageStore,
    provider_surface: &str,
    cursor_key: &str,
    totals: RawTokenTotals,
    version: &str,
) -> Result<()> {
    let cursor_value = serde_json::to_string(&totals)?;
    store.set_provider_cursor(
        provider_surface,
        cursor_key,
        &cursor_value,
        version,
        version,
    )
}

fn parse_period_start(period_start: &str) -> Result<OffsetDateTime> {
    if let Ok(value) =
        OffsetDateTime::parse(period_start, &time::format_description::well_known::Rfc3339)
    {
        return Ok(value);
    }
    if let Ok(date) = Date::parse(
        period_start,
        time::macros::format_description!("[year]-[month]-[day]"),
    ) {
        return Ok(PrimitiveDateTime::new(date, Time::MIDNIGHT).assume_offset(UtcOffset::UTC));
    }
    if let Some(date) = parse_short_month_date(period_start) {
        return Ok(PrimitiveDateTime::new(date, Time::MIDNIGHT).assume_offset(UtcOffset::UTC));
    }
    Err(GlorpError::Message(format!(
        "invalid usage period_start: {period_start}"
    )))
}

/// Parse the human-readable date format the ccusage-codex helper emits, e.g. "Mar 23, 2026".
fn parse_short_month_date(s: &str) -> Option<Date> {
    let (day_month, year_str) = s.split_once(',')?;
    let year: i32 = year_str.trim().parse().ok()?;
    let (month_str, day_str) = day_month.trim().split_once(' ')?;
    let day: u8 = day_str.trim().parse().ok()?;
    let month = match month_str {
        "Jan" => Month::January,
        "Feb" => Month::February,
        "Mar" => Month::March,
        "Apr" => Month::April,
        "May" => Month::May,
        "Jun" => Month::June,
        "Jul" => Month::July,
        "Aug" => Month::August,
        "Sep" => Month::September,
        "Oct" => Month::October,
        "Nov" => Month::November,
        "Dec" => Month::December,
        _ => return None,
    };
    Date::from_calendar_date(year, month, day).ok()
}

#[cfg(test)]
mod ccusage_major_version_tests {
    use super::*;

    #[test]
    fn parses_name_prefixed_and_bare_and_v_prefixed_versions() {
        assert_eq!(ccusage_major_version("ccusage 20.0.6"), Some(20));
        assert_eq!(ccusage_major_version("18.0.11"), Some(18));
        assert_eq!(ccusage_major_version("v21.1.0"), Some(21));
    }

    #[test]
    fn unparseable_versions_stay_on_the_legacy_invocation() {
        assert_eq!(ccusage_major_version("unknown"), None);
        assert_eq!(ccusage_major_version(""), None);
        assert_eq!(ccusage_major_version("ccusage beta"), None);
    }
}

#[cfg(test)]
mod parse_period_start_tests {
    use super::*;

    #[test]
    fn rfc3339_format_is_accepted() {
        let parsed = parse_period_start("2026-03-23T14:00:00Z").unwrap();
        assert_eq!(parsed.year(), 2026);
        assert_eq!(parsed.month(), Month::March);
        assert_eq!(parsed.day(), 23);
    }

    #[test]
    fn iso_date_only_is_accepted() {
        let parsed = parse_period_start("2026-03-23").unwrap();
        assert_eq!(parsed.year(), 2026);
        assert_eq!(parsed.month(), Month::March);
        assert_eq!(parsed.day(), 23);
        assert_eq!(parsed.hour(), 0);
    }

    #[test]
    fn codex_short_month_format_is_accepted() {
        let parsed = parse_period_start("Mar 23, 2026").unwrap();
        assert_eq!(parsed.year(), 2026);
        assert_eq!(parsed.month(), Month::March);
        assert_eq!(parsed.day(), 23);
    }

    #[test]
    fn codex_short_month_handles_single_digit_day() {
        let parsed = parse_period_start("Apr 5, 2026").unwrap();
        assert_eq!(parsed.month(), Month::April);
        assert_eq!(parsed.day(), 5);
    }

    #[test]
    fn unknown_format_returns_error() {
        assert!(parse_period_start("not a date").is_err());
        assert!(parse_period_start("2026/03/23").is_err());
        assert!(parse_period_start("Marz 23, 2026").is_err());
    }
}

#[cfg(test)]
mod run_command_timeout_tests {
    use super::*;
    use std::time::Instant;

    /// `sleep 5` started with a 100ms budget must be killed and return
    /// `ErrorKind::TimedOut` quickly. The hard ceiling here (2s) is what
    /// the watch loop relies on to avoid wedging on a hung helper.
    #[test]
    fn timeout_kills_long_running_child() {
        let mut command = Command::new("sleep");
        command.arg("5");
        let started = Instant::now();
        let result = run_command_with_timeout(&mut command, Duration::from_millis(100));
        let elapsed = started.elapsed();

        match result {
            Err(GlorpError::Io(err)) => assert_eq!(err.kind(), io::ErrorKind::TimedOut),
            other => panic!("expected TimedOut io error, got {other:?}"),
        }
        assert!(
            elapsed < Duration::from_secs(2),
            "timeout path should not block past the configured timeout (took {elapsed:?})"
        );
    }

    /// A child that exits well within the timeout returns its captured output.
    #[test]
    fn fast_child_returns_output() {
        let mut command = Command::new("sh");
        command.args(["-c", "printf hello"]);
        let output = run_command_with_timeout(&mut command, Duration::from_secs(5))
            .expect("fast child should succeed");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"hello");
    }

    /// Helpers can emit enough JSON to fill an OS pipe. The runner must drain
    /// stdout while waiting so a fast, chatty helper does not look hung.
    #[test]
    fn large_stdout_child_returns_output() {
        let mut command = Command::new("sh");
        command.args(["-c", "yes agentsview-json-line | head -n 9000"]);
        let output = run_command_with_timeout(&mut command, Duration::from_secs(2))
            .expect("large stdout child should not deadlock on a full pipe");

        assert!(output.status.success());
        assert!(output.stdout.len() > 100_000);
    }
}
