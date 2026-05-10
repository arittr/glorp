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
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use time::{Date, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

const CLAUDE_SURFACE: &str = "claude-code";
const CODEX_SURFACE: &str = "codex";
const CONFIDENCE: &str = "local-log-derived";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperCommand {
    pub program: PathBuf,
    pub args_prefix: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HelperPaths {
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
    weights: EffectiveTokenWeights,
}

enum HelperInvocation {
    Records {
        version: String,
        records: Vec<NormalizedUsageRecord>,
    },
    EarlyExit {
        diagnostics: Vec<ProviderDiagnostic>,
    },
}

impl CcusageCommandProvider {
    pub fn new(helpers: HelperPaths) -> Self {
        Self {
            helpers,
            weights: EffectiveTokenWeights::default(),
        }
    }

    pub fn with_weights(mut self, weights: EffectiveTokenWeights) -> Self {
        self.weights = weights;
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
            return Ok(HelperInvocation::EarlyExit {
                diagnostics: vec![diagnostic],
            });
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
                return Ok(HelperInvocation::EarlyExit {
                    diagnostics: vec![diagnostic],
                });
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

        let output = self.run_command(provider_surface, &helper_command, daily_args)?;
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let diagnostic = diagnostic(
                provider_surface,
                "helper_exit",
                &format!("{command_name} exited with status {code}"),
            );
            persist_diagnostic(store, &diagnostic)?;
            return Ok(HelperInvocation::EarlyExit {
                diagnostics: vec![diagnostic],
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let records = match normalize_usage_json(provider_surface, &stdout) {
            Ok(records) => records,
            Err(diagnostic) => {
                persist_diagnostic(store, &diagnostic)?;
                return Ok(HelperInvocation::EarlyExit {
                    diagnostics: vec![diagnostic],
                });
            }
        };

        Ok(HelperInvocation::Records { version, records })
    }

    fn poll_helper(
        &self,
        store: &mut UsageStore,
        provider_surface: &str,
        command_name: &str,
        helper: Option<&Path>,
        daily_args: &[&str],
    ) -> Result<UsagePollResult> {
        let (version, records) =
            match self.invoke_helper(store, provider_surface, command_name, helper, daily_args)? {
                HelperInvocation::Records { version, records } => (version, records),
                HelperInvocation::EarlyExit { diagnostics } => {
                    return Ok(UsagePollResult {
                        deltas: Vec::new(),
                        diagnostics,
                        total_effective_tokens: 0.0,
                    });
                }
            };

        let mut deltas = Vec::new();
        let mut diagnostics = Vec::new();
        let weights = self.weights;
        let observed_at = OffsetDateTime::now_utc();
        for record in records {
            let parsed_period_start = match parse_period_start(&record.period_start) {
                Ok(parsed) => parsed,
                Err(_) => {
                    let diagnostic = diagnostic(
                        provider_surface,
                        "invalid_period_start",
                        &format!("{provider_surface} returned invalid_period_start"),
                    );
                    persist_diagnostic(store, &diagnostic)?;
                    diagnostics.push(diagnostic);
                    continue;
                }
            };

            let key = ProviderCursorKey {
                provider_surface: record.provider_surface.clone(),
                command: command_name.to_string(),
                source_surface: "daily".to_string(),
                period_start: record.period_start.clone(),
                model: record.model.clone(),
            };

            let cursor_key = cursor_key(&key)?;
            let previous_raw = match store.provider_cursor(provider_surface, &cursor_key) {
                Ok(Some(value)) => Some(value),
                Ok(None) => match read_legacy_cursor_value(
                    store,
                    provider_surface,
                    command_name,
                    &record.period_start,
                    record.model.clone(),
                    &version,
                ) {
                    Ok(Some(value)) => {
                        store.set_provider_cursor(
                            provider_surface,
                            &cursor_key,
                            &value,
                            &version,
                            &version,
                        )?;
                        Some(value)
                    }
                    Ok(None) => None,
                    Err(_) => {
                        let diagnostic =
                            diagnostic(provider_surface, "cursor_corruption", "cursor_corruption");
                        persist_diagnostic(store, &diagnostic)?;
                        diagnostics.push(diagnostic);
                        None
                    }
                },
                Err(_) => {
                    let diagnostic =
                        diagnostic(provider_surface, "cursor_corruption", "cursor_corruption");
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
                            diagnostic(provider_surface, "cursor_corruption", "cursor_corruption");
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
                    "cursor_total_decreased for provider cursor",
                );
                persist_diagnostic(store, &diagnostic)?;
                diagnostics.push(diagnostic);
                // cursor_total_decreased is a hard reset (not a normal advance).
                write_cursor(
                    store,
                    provider_surface,
                    &cursor_key,
                    record.raw_totals,
                    &version,
                )?;
                continue;
            };

            if !delta_totals.has_positive_effective_bucket() {
                // Empty delta: advance the cursor so subsequent polls do not re-read the
                // same totals. Nothing to feed the pet, no row to stage.
                write_cursor(
                    store,
                    provider_surface,
                    &cursor_key,
                    record.raw_totals,
                    &version,
                )?;
                continue;
            }

            let effective_tokens = delta_totals.effective_tokens(weights);
            let cursor_value = serde_json::to_string(&record.raw_totals)?;
            let cursor_update = ProviderCursorUpdate {
                provider_surface: provider_surface.to_string(),
                cursor_key: cursor_key.clone(),
                cursor_value,
                provider_version: version.clone(),
                parser_version: version.clone(),
            };
            // Advance the cursor so subsequent polls do not re-emit the same delta.
            // The command path stages the delta into the unapplied ledger via
            // `stage_usage_poll_deltas` (which smears across buckets) before applying
            // and saving pet state. If apply or save fails, the unapplied rows remain
            // and the next successful run reapplies them via `apply_unapplied_usage`.
            write_cursor(
                store,
                provider_surface,
                &cursor_key,
                record.raw_totals,
                &version,
            )?;
            deltas.push(UsageDelta {
                provider_surface: record.provider_surface,
                command: command_name.to_string(),
                effective_tokens,
                confidence: CONFIDENCE.to_string(),
                period_start: parsed_period_start,
                observed_at,
                model: record.model,
                cursor_update,
            });
        }

        let total_effective_tokens = deltas.iter().map(|delta| delta.effective_tokens).sum();
        Ok(UsagePollResult {
            deltas,
            diagnostics,
            total_effective_tokens,
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
        let (version, records) =
            match self.invoke_helper(store, provider_surface, command_name, helper, daily_args)? {
                HelperInvocation::Records { version, records } => (version, records),
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
        let weights = self.weights;
        for record in records {
            let parsed_period_start = match parse_period_start(&record.period_start) {
                Ok(parsed) => parsed,
                Err(_) => {
                    let diagnostic = diagnostic(
                        provider_surface,
                        "invalid_period_start",
                        &format!("{provider_surface} returned invalid_period_start"),
                    );
                    persist_diagnostic(store, &diagnostic)?;
                    diagnostics.push(diagnostic);
                    continue;
                }
            };

            let effective_tokens = record.raw_totals.effective_tokens(weights);
            daily_usage.push(
                crate::game::calibration::DailyUsage::with_activity_timestamp(
                    parsed_period_start,
                    effective_tokens,
                ),
            );

            let key = ProviderCursorKey {
                provider_surface: record.provider_surface.clone(),
                command: command_name.to_string(),
                source_surface: "daily".to_string(),
                period_start: record.period_start.clone(),
                model: record.model.clone(),
            };
            let cursor_key = cursor_key(&key)?;
            let cursor_value = serde_json::to_string(&record.raw_totals)?;
            cursor_updates.push(ProviderCursorUpdate {
                provider_surface: provider_surface.to_string(),
                cursor_key,
                cursor_value,
                provider_version: version.clone(),
                parser_version: version.clone(),
            });
        }

        Ok(UsageSnapshot {
            daily_usage,
            cursor_updates,
            diagnostics,
        })
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
        command.output().map_err(GlorpError::from)
    }
}

impl UsageProvider for CcusageCommandProvider {
    fn poll(&self, store: &mut UsageStore) -> Result<UsagePollResult> {
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
        let mut diagnostics = claude.diagnostics;
        diagnostics.extend(codex.diagnostics);
        let total_effective_tokens = deltas.iter().map(|delta| delta.effective_tokens).sum();
        Ok(UsagePollResult {
            deltas,
            diagnostics,
            total_effective_tokens,
        })
    }

    fn snapshot_for_calibration(&self, store: &mut UsageStore) -> Result<UsageSnapshot> {
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
        let mut diagnostics = claude.diagnostics;
        diagnostics.extend(codex.diagnostics);

        Ok(UsageSnapshot {
            daily_usage,
            cursor_updates,
            diagnostics,
        })
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
        Self {
            claude,
            codex,
            node,
        }
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
    let date = Date::parse(
        period_start,
        time::macros::format_description!("[year]-[month]-[day]"),
    )
    .map_err(|err| GlorpError::Message(format!("invalid usage period_start: {err}")))?;
    Ok(PrimitiveDateTime::new(date, Time::MIDNIGHT).assume_offset(UtcOffset::UTC))
}
