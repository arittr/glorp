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
        "gemini" | "kimi" | "opencode" | "copilot" => lower,
        other => other.to_string(),
    };
    let display_name = trimmed.to_string();
    (provider_surface, display_name)
}

fn unknown_source_hash(raw: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in raw.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

impl SourceIdentity {
    fn new(provider_surface: String, display_name: String, raw_agent: Option<String>) -> Self {
        let source_family = match provider_surface.as_str() {
            "claude-code" | "codex" => SourceFamily::KnownCodingAgent,
            _ => SourceFamily::UnknownCodingAgent,
        };
        Self {
            provider_surface,
            display_name,
            raw_agent,
            source_family,
        }
    }

    pub fn from_raw_agent(raw: &str) -> Self {
        let (provider_surface, display_name) = normalize_source_label(raw);
        if !matches!(
            provider_surface.as_str(),
            "claude-code" | "codex" | "gemini" | "kimi" | "opencode" | "copilot" | "unknown"
        ) {
            let safe_label = format!("unknown-agent-{}", unknown_source_hash(raw));
            return Self::new(safe_label.clone(), safe_label, None);
        }
        Self::new(provider_surface, display_name, Some(raw.to_string()))
    }

    pub fn from_provider_surface(surface: &str) -> Self {
        let (provider_surface, display_name) = normalize_source_label(surface);
        Self::new(provider_surface, display_name, None)
    }

    pub fn from_tokenmaxxing_source(surface: &str) -> Self {
        let normalized = surface.trim().to_ascii_lowercase();
        let provider_surface = match normalized.as_str() {
            "claude" | "claude-code" => "claude".to_string(),
            "codex" | "ccusage-codex" => "codex".to_string(),
            other if other.is_empty() || other == "all" => "unknown".to_string(),
            other => other.to_string(),
        };
        let source_family = match provider_surface.as_str() {
            "claude" | "codex" => SourceFamily::KnownCodingAgent,
            _ => SourceFamily::UnknownCodingAgent,
        };
        Self {
            display_name: provider_surface.clone(),
            provider_surface,
            raw_agent: Some(surface.to_string()),
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
