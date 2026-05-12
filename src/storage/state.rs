use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::OffsetDateTime;

use crate::game::{calibration::CalibrationBaseline, metabolism::RhythmProfile};

const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PetState {
    pub schema_version: u32,
    pub pet: PetIdentity,
    pub stage: String,
    pub xp: f64,
    pub lifetime_effective_tokens: f64,
    pub vitals: Vitals,
    #[serde(default)]
    pub calibration: CalibrationBaseline,
    #[serde(default)]
    pub rhythm: RhythmProfile,
    #[serde(default)]
    pub seen_stage_transitions: Vec<String>,
    #[serde(default)]
    pub recent_events: Vec<String>,
    pub created_at: OffsetDateTime,
    pub last_updated_at: OffsetDateTime,
    pub last_usage_poll_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PetIdentity {
    pub seed: String,
    pub generated_species: String,
    pub accepted_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vitals {
    pub fed: f64,
    pub happiness: f64,
    pub energy: f64,
}

pub struct StateStore {
    path: PathBuf,
}

impl PetState {
    pub fn new_for_test(seed: &str, name: &str) -> Self {
        let now = OffsetDateTime::UNIX_EPOCH;
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            pet: PetIdentity {
                seed: seed.to_string(),
                generated_species: "fuzz".to_string(),
                accepted_name: name.to_string(),
            },
            stage: "hatchling".to_string(),
            xp: 0.0,
            lifetime_effective_tokens: 0.0,
            calibration: CalibrationBaseline::default(),
            rhythm: RhythmProfile::default(),
            seen_stage_transitions: Vec::new(),
            recent_events: Vec::new(),
            vitals: Vitals {
                fed: 70.0,
                happiness: 70.0,
                energy: 70.0,
            },
            created_at: now,
            last_updated_at: now,
            last_usage_poll_at: None,
        }
    }
}

impl StateStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> crate::error::Result<Option<PetState>> {
        if !self.path.exists() {
            return Ok(None);
        }

        let text = std::fs::read_to_string(&self.path)?;
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|err| {
            crate::error::GlorpError::Message(format!(
                "malformed state.json at {}: {err}",
                self.path.display()
            ))
        })?;

        let schema_version = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                crate::error::GlorpError::Message(
                    "malformed state.json: missing numeric schema_version".into(),
                )
            })?;

        if schema_version != u64::from(CURRENT_SCHEMA_VERSION) {
            return Err(crate::error::GlorpError::Message(format!(
                "unsupported schema version {schema_version} in state.json"
            )));
        }

        serde_json::from_value(value).map(Some).map_err(|err| {
            crate::error::GlorpError::Message(format!(
                "malformed state.json at {}: {err}",
                self.path.display()
            ))
        })
    }

    pub fn save(&self, state: &PetState) -> crate::error::Result<()> {
        if state.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(crate::error::GlorpError::Message(format!(
                "unsupported schema version {} in state save",
                state.schema_version
            )));
        }

        let parent = self.path.parent().ok_or_else(|| {
            crate::error::GlorpError::Message(format!(
                "state path has no parent directory: {}",
                self.path.display()
            ))
        })?;
        std::fs::create_dir_all(parent)?;

        let text = serde_json::to_string_pretty(state)?;

        // Atomic save: write to a sibling temp file in the same directory, fsync
        // its contents to disk, then rename into place. POSIX `rename(2)` is
        // atomic on the same filesystem; `NamedTempFile::persist` uses
        // `MoveFileExA` on Windows, which is atomic for same-volume moves.
        // `NamedTempFile` auto-deletes the temp file on drop if persist fails,
        // so the directory is not littered with `state.json.tmp.*` files when
        // any step errors out.
        let mut tmp = tempfile::Builder::new()
            .prefix("state.json.tmp.")
            .tempfile_in(parent)?;
        {
            use std::io::Write;
            tmp.as_file_mut().write_all(text.as_bytes())?;
            tmp.as_file_mut().sync_all()?;
        }
        tmp.persist(&self.path).map_err(|err| err.error)?;
        Ok(())
    }

    pub fn delete(&self) -> crate::error::Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

#[doc(hidden)]
pub struct PetStateFixture {
    state: PetState,
}

impl PetStateFixture {
    pub fn named(name: &str) -> Self {
        Self {
            state: PetState::new_for_test("fixture-seed", name),
        }
    }

    pub fn with_stage(mut self, stage: &str) -> Self {
        self.state.stage = stage.to_string();
        self
    }

    pub fn with_recent_event(mut self, event: &str) -> Self {
        self.state.recent_events.push(event.to_string());
        self
    }

    fn into_state(self) -> PetState {
        self.state
    }
}

#[doc(hidden)]
pub fn write_state_for_test(
    dir: &std::path::Path,
    fixture: PetStateFixture,
) -> crate::error::Result<()> {
    let paths = crate::paths::AppPaths::from_config_dir(dir.to_path_buf());
    paths.ensure()?;
    StateStore::new(paths.state_file).save(&fixture.into_state())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn tmp_files_in(dir: &std::path::Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("state.json.tmp."))
            })
            .collect()
    }

    #[test]
    fn save_leaves_no_temp_files_behind() {
        let dir = tempdir().unwrap();
        let store = StateStore::new(dir.path().join("state.json"));
        store
            .save(&PetState::new_for_test("seed-1", "buddy"))
            .unwrap();
        assert!(
            tmp_files_in(dir.path()).is_empty(),
            "save should not leak temp files: {:?}",
            tmp_files_in(dir.path())
        );
    }

    #[test]
    fn load_ignores_stray_temp_files_in_the_state_directory() {
        let dir = tempdir().unwrap();
        let store = StateStore::new(dir.path().join("state.json"));
        let original = PetState::new_for_test("seed-2", "kit");
        store.save(&original).unwrap();

        // Hand-write a leftover that resembles a crash mid-save from a previous
        // run. `load()` must read `state.json` and not be tripped by siblings.
        let stray = dir.path().join("state.json.tmp.99999");
        std::fs::write(&stray, b"garbage that is not json").unwrap();

        let loaded = store.load().unwrap().expect("state.json should load");
        assert_eq!(loaded, original);
        assert!(stray.exists(), "load must not touch foreign temp files");
    }

    #[test]
    fn load_rejects_corrupted_destination_file() {
        // This documents the invariant the atomic save protects: if the
        // destination file is ever partially written, `load` surfaces an error
        // rather than returning a half-state. Atomic rename ensures the real
        // pipeline never produces such a file, but the detector must stay sharp.
        let dir = tempdir().unwrap();
        let store = StateStore::new(dir.path().join("state.json"));
        store
            .save(&PetState::new_for_test("seed-3", "scrap"))
            .unwrap();
        std::fs::write(dir.path().join("state.json"), b"{\"schema_versi").unwrap();

        let err = store.load().expect_err("corrupted state.json must error");
        assert!(
            matches!(err, crate::error::GlorpError::Message(ref msg) if msg.contains("malformed state.json")),
            "expected malformed-json error, got: {err:?}"
        );
    }
}
