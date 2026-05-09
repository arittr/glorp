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

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(state)?;
        std::fs::write(&self.path, text)?;
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
