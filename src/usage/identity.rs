use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceFamily {
    KnownCodingAgent,
    UnknownCodingAgent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceIdentity {
    /// Stable storage/provider key: lowercase, short, safe to display truncated.
    pub provider_surface: String,
    /// User-facing label; may preserve capitalization.
    pub display_name: String,
    /// Raw `ccusage` source/agent value when the JSON provides one.
    pub raw_agent: Option<String>,
    pub source_family: SourceFamily,
}

pub fn normalize_source_label(raw: &str) -> (String, String) {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
        return ("unknown".to_string(), "unknown".to_string());
    }
    let lower = trimmed.to_ascii_lowercase();
    let provider_surface = match lower.as_str() {
        "claude" | "claude-code" => "claude-code".to_string(),
        "codex" | "ccusage-codex" => "codex".to_string(),
        other => other.to_string(),
    };
    let display_name = trimmed.to_string();
    (provider_surface, display_name)
}

impl SourceIdentity {
    pub fn from_raw_agent(raw: &str) -> Self {
        let (provider_surface, display_name) = normalize_source_label(raw);
        let source_family = match provider_surface.as_str() {
            "claude-code" | "codex" => SourceFamily::KnownCodingAgent,
            _ => SourceFamily::UnknownCodingAgent,
        };
        Self {
            provider_surface,
            display_name,
            raw_agent: Some(raw.to_string()),
            source_family,
        }
    }

    pub fn from_provider_surface(surface: &str) -> Self {
        let (provider_surface, display_name) = normalize_source_label(surface);
        let source_family = match provider_surface.as_str() {
            "claude-code" | "codex" => SourceFamily::KnownCodingAgent,
            _ => SourceFamily::UnknownCodingAgent,
        };
        Self {
            provider_surface,
            display_name,
            raw_agent: None,
            source_family,
        }
    }

    pub fn claude_code() -> Self {
        Self::from_provider_surface("claude-code")
    }

    pub fn codex() -> Self {
        Self::from_provider_surface("codex")
    }
}
