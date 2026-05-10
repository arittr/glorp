use crate::error::{GlorpError, Result};
use crate::game::effective_tokens::EffectiveTokenWeights;
use crate::storage::usage_store::{
    NormalizedUsageEvent, ProviderDiagnostic as StoredProviderDiagnostic, UsageStore,
};
use crate::usage::normalize::{normalize_usage_json, RawTokenTotals};
use crate::usage::provider::{
    ProviderCursorKey, ProviderDiagnostic, UsageDelta, UsagePollResult, UsageProvider,
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
}

impl CcusageCommandProvider {
    pub fn new(helpers: HelperPaths) -> Self {
        Self { helpers }
    }

    pub fn from_environment() -> Self {
        Self::new(HelperDiscovery::discover().into())
    }

    fn poll_helper(
        &self,
        store: &mut UsageStore,
        provider_surface: &str,
        command_name: &str,
        helper: Option<&Path>,
        daily_args: &[&str],
    ) -> Result<UsagePollResult> {
        let Some(helper) = helper else {
            let diagnostic = diagnostic(
                provider_surface,
                "missing_helper",
                &format!("{command_name} helper was not found"),
            );
            persist_diagnostic(store, &diagnostic)?;
            return Ok(UsagePollResult {
                deltas: Vec::new(),
                diagnostics: vec![diagnostic],
                total_effective_tokens: 0.0,
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
                return Ok(UsagePollResult {
                    deltas: Vec::new(),
                    diagnostics: vec![diagnostic],
                    total_effective_tokens: 0.0,
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
            return Ok(UsagePollResult {
                deltas: Vec::new(),
                diagnostics: vec![diagnostic],
                total_effective_tokens: 0.0,
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let records = match normalize_usage_json(provider_surface, &stdout) {
            Ok(records) => records,
            Err(diagnostic) => {
                persist_diagnostic(store, &diagnostic)?;
                return Ok(UsagePollResult {
                    deltas: Vec::new(),
                    diagnostics: vec![diagnostic],
                    total_effective_tokens: 0.0,
                });
            }
        };

        let mut deltas = Vec::new();
        let mut diagnostics = Vec::new();
        let weights = EffectiveTokenWeights::default();
        let observed_at = OffsetDateTime::now_utc();
        for record in records {
            let key = ProviderCursorKey {
                provider_surface: record.provider_surface.clone(),
                command: command_name.to_string(),
                parser_version: version.clone(),
                period_start: record.period_start.clone(),
                model: record.model.clone(),
            };

            let cursor_key = cursor_key(&key)?;
            let previous = match store.provider_cursor(provider_surface, &cursor_key) {
                Ok(Some(previous)) => match serde_json::from_str::<RawTokenTotals>(&previous) {
                    Ok(previous) => previous,
                    Err(_) => {
                        let diagnostic =
                            diagnostic(provider_surface, "cursor_corruption", "cursor_corruption");
                        persist_diagnostic(store, &diagnostic)?;
                        diagnostics.push(diagnostic);
                        RawTokenTotals::default()
                    }
                },
                Ok(None) => RawTokenTotals::default(),
                Err(_) => {
                    let diagnostic =
                        diagnostic(provider_surface, "cursor_corruption", "cursor_corruption");
                    persist_diagnostic(store, &diagnostic)?;
                    diagnostics.push(diagnostic);
                    RawTokenTotals::default()
                }
            };

            let Some(delta_totals) = record.raw_totals.positive_delta_since(previous) else {
                let diagnostic = diagnostic(
                    provider_surface,
                    "cursor_total_decreased",
                    "cursor_total_decreased for provider cursor",
                );
                persist_diagnostic(store, &diagnostic)?;
                diagnostics.push(diagnostic);
                write_cursor(
                    store,
                    provider_surface,
                    &cursor_key,
                    record.raw_totals,
                    &version,
                )?;
                continue;
            };

            write_cursor(
                store,
                provider_surface,
                &cursor_key,
                record.raw_totals,
                &version,
            )?;
            if !delta_totals.has_positive_effective_bucket() {
                continue;
            }

            let effective_tokens = delta_totals.effective_tokens(weights);
            store.insert_event(&NormalizedUsageEvent {
                provider_surface: record.provider_surface.clone(),
                provider_version: version.clone(),
                parser_version: version.clone(),
                command: command_name.to_string(),
                source_surface: "daily".to_string(),
                period_start: parse_period_start(&record.period_start)?,
                observed_at,
                bucket_at: observed_at,
                model: record.model.clone(),
                input_tokens: delta_totals.uncached_input as f64,
                output_tokens: delta_totals.output as f64,
                cache_creation_tokens: delta_totals.cache_creation as f64,
                cache_read_tokens: delta_totals.cache_read as f64,
                reasoning_output_tokens: delta_totals.reasoning_output as f64,
                effective_tokens,
                cost_usd: record.display_cost_usd,
                confidence: CONFIDENCE.to_string(),
            })?;
            deltas.push(UsageDelta {
                provider_surface: record.provider_surface,
                effective_tokens,
                confidence: CONFIDENCE.to_string(),
                period_start: record.period_start,
                model: record.model,
            });
        }

        let total_effective_tokens = deltas.iter().map(|delta| delta.effective_tokens).sum();
        Ok(UsagePollResult {
            deltas,
            diagnostics,
            total_effective_tokens,
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
