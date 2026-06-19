use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const HELPER_LOCATOR_FILE: &str = "helper-locator.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperLocator {
    pub agentsview_bin: Option<PathBuf>,
    pub ccusage_bin: Option<PathBuf>,
    pub ccusage_codex_bin: Option<PathBuf>,
    pub node_bin: Option<PathBuf>,
}

impl HelperLocator {
    pub fn from_current_environment() -> Self {
        Self {
            agentsview_bin: std::env::var_os("GLORP_AGENTSVIEW_BIN").map(PathBuf::from),
            ccusage_bin: std::env::var_os("GLORP_CCUSAGE_BIN").map(PathBuf::from),
            ccusage_codex_bin: std::env::var_os("GLORP_CCUSAGE_CODEX_BIN").map(PathBuf::from),
            node_bin: std::env::var_os("GLORP_NODE_BIN").map(PathBuf::from),
        }
    }

    pub fn has_any_path(&self) -> bool {
        self.agentsview_bin.is_some()
            || self.ccusage_bin.is_some()
            || self.ccusage_codex_bin.is_some()
            || self.node_bin.is_some()
    }
}

pub fn write_helper_locator(path: &Path, locator: &HelperLocator) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("json.tmp");
    {
        let mut tmp = std::fs::File::create(&tmp_path)?;
        tmp.write_all(serde_json::to_string_pretty(locator)?.as_bytes())?;
        tmp.sync_all()?;
    }
    std::fs::rename(tmp_path, path)?;
    Ok(())
}

pub fn read_helper_locator(path: &Path) -> Result<Option<HelperLocator>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}
