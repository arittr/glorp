use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_cache_read_weight")]
    pub cache_read_weight: f64,
    /// Multiplier on the per-provider discontinuity threshold
    /// (`guard_ratio x baseline x days_factor`, floored at 50M effective
    /// tokens). The escape hatch for users the guard refuses honestly.
    #[serde(default = "default_discontinuity_guard_ratio")]
    pub discontinuity_guard_ratio: f64,
}

fn default_cache_read_weight() -> f64 {
    0.03
}

fn default_discontinuity_guard_ratio() -> f64 {
    crate::game::runtime::DISCONTINUITY_GUARD_RATIO
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            cache_read_weight: default_cache_read_weight(),
            discontinuity_guard_ratio: default_discontinuity_guard_ratio(),
        }
    }
}

impl AppConfig {
    pub fn load_or_default(path: &Path) -> crate::error::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let text = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&text).map_err(|err| {
            crate::error::GlorpError::Message(format!("malformed config.toml: {err}"))
        })?;

        if !(0.0..=1.0).contains(&config.cache_read_weight) {
            return Err(crate::error::GlorpError::Message(
                "cache_read_weight must be between 0.0 and 1.0".into(),
            ));
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discontinuity_guard_ratio_defaults_when_absent_from_config() {
        let config: AppConfig = toml::from_str("cache_read_weight = 0.05").unwrap();
        assert_eq!(
            config.discontinuity_guard_ratio,
            crate::game::runtime::DISCONTINUITY_GUARD_RATIO
        );
        assert_eq!(AppConfig::default().discontinuity_guard_ratio, 5.0);
    }

    #[test]
    fn discontinuity_guard_ratio_is_overridable() {
        let config: AppConfig = toml::from_str("discontinuity_guard_ratio = 12.5").unwrap();
        assert_eq!(config.discontinuity_guard_ratio, 12.5);
        assert_eq!(config.cache_read_weight, 0.03);
    }
}
