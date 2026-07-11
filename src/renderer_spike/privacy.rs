use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{GlorpError, Result};

const FORBIDDEN_TOKENS: &[&str] = &[
    "source_name",
    "display_name",
    "client-secret-project",
    "/users/",
    "/tmp/",
    "prompt",
    "response",
    "transcript",
    "tool payload",
    "diagnostic",
    "very-secret-seed",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivacyScanArtifact {
    pub schema_version: u16,
    pub passed: bool,
    pub files_scanned: u64,
    pub rejected_tokens: Vec<String>,
}

pub fn scan_owned_directory(root: &Path) -> Result<PrivacyScanArtifact> {
    let mut files_scanned = 0_u64;
    let mut rejected_tokens = Vec::new();
    scan_directory(root, &mut files_scanned, &mut rejected_tokens)?;
    rejected_tokens.sort();
    rejected_tokens.dedup();
    Ok(PrivacyScanArtifact {
        schema_version: 1,
        passed: rejected_tokens.is_empty(),
        files_scanned,
        rejected_tokens,
    })
}

pub fn write_privacy_scan(root: &Path) -> Result<()> {
    let scan = scan_owned_directory(root)?;
    super::artifacts::write_json(&root.join("privacy-scan.json"), &scan)?;
    if !scan.passed {
        return Err(GlorpError::Message(format!(
            "renderer spike privacy scan rejected tokens: {}",
            scan.rejected_tokens.join(", ")
        )));
    }
    Ok(())
}

fn scan_directory(
    root: &Path,
    files_scanned: &mut u64,
    rejected_tokens: &mut Vec<String>,
) -> Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path
            .file_name()
            .is_some_and(|name| name == "privacy-scan.json")
        {
            continue;
        }
        if path.is_dir() {
            scan_directory(&path, files_scanned, rejected_tokens)?;
            continue;
        }
        *files_scanned = files_scanned.saturating_add(1);
        let bytes = std::fs::read(&path)?;
        let value = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
        for token in FORBIDDEN_TOKENS {
            if value.contains(token) {
                rejected_tokens.push((*token).to_string());
            }
        }
    }
    Ok(())
}
