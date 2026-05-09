use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_cache_read_weight")]
    pub cache_read_weight: f64,
}

fn default_cache_read_weight() -> f64 {
    0.03
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            cache_read_weight: default_cache_read_weight(),
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
