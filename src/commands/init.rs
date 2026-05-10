use crate::{
    config::AppConfig,
    error::{GlorpError, Result},
    game::{
        calibration::CalibrationBaseline, effective_tokens::EffectiveTokenWeights,
        metabolism::RhythmProfile,
    },
    paths::AppPaths,
    pet::generation::{generate_pet, resolve_accepted_name},
    storage::{
        state::{PetIdentity, PetState, StateStore, Vitals},
        usage_store::UsageStore,
    },
    usage::{ccusage::CcusageCommandProvider, provider::UsageProvider},
};
use std::io::{self, IsTerminal, Write};
use time::OffsetDateTime;

pub fn run(seed: Option<String>, name: Option<String>, yes: bool) -> Result<()> {
    let paths = AppPaths::resolve()?;
    paths.ensure()?;
    let store = StateStore::new(paths.state_file.clone());
    if store.load()?.is_some() && !yes {
        return Err(GlorpError::Message(
            "glorp already has a pet; pass --yes to replace pet state".into(),
        ));
    }

    let seed = seed
        .unwrap_or_else(|| format!("glorp-{}", OffsetDateTime::now_utc().unix_timestamp_nanos()));
    let generated = generate_pet(&seed);
    println!("generated name: {}", generated.generated_name);
    let accepted_name = choose_name(&generated.generated_name, name)?;

    let mut calibration = CalibrationBaseline::default();
    let mut rhythm = RhythmProfile::default();
    if let Ok(mut usage_store) = UsageStore::open(&paths.usage_db) {
        let config = AppConfig::load_or_default(&paths.config_file).unwrap_or_default();
        let weights = EffectiveTokenWeights::from_config(config);
        if let Ok(snapshot) = CcusageCommandProvider::from_environment_with_weights(weights)
            .snapshot_for_calibration(&mut usage_store)
        {
            calibration = CalibrationBaseline::from_history(&snapshot.daily_usage);
            rhythm = RhythmProfile::from_history(&snapshot.daily_usage);
            usage_store.advance_cursors(snapshot.cursor_updates, OffsetDateTime::now_utc())?;
        }
    }

    let now = OffsetDateTime::now_utc();
    let state = PetState {
        schema_version: 1,
        pet: PetIdentity {
            seed,
            generated_species: generated.species.as_str().to_string(),
            accepted_name: accepted_name.clone(),
        },
        stage: "s0".into(),
        xp: 0.0,
        lifetime_effective_tokens: 0.0,
        vitals: Vitals {
            fed: 70.0,
            happiness: 70.0,
            energy: 70.0,
        },
        calibration,
        rhythm,
        seen_stage_transitions: Vec::new(),
        recent_events: vec![format!("{accepted_name} has hatched")],
        created_at: now,
        last_updated_at: now,
        last_usage_poll_at: None,
    };
    store.save(&state)?;
    println!("{accepted_name} has hatched");
    Ok(())
}

fn choose_name(generated_name: &str, replacement: Option<String>) -> Result<String> {
    if replacement.is_some() {
        return Ok(resolve_accepted_name(
            generated_name,
            replacement.as_deref(),
        ));
    }

    if !io::stdin().is_terminal() {
        return Ok(resolve_accepted_name(generated_name, None));
    }

    print!("Use this name? [Y/n] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let answer = answer.trim();
    if answer.is_empty() || answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes") {
        return Ok(generated_name.to_string());
    }

    print!("Replacement name: ");
    io::stdout().flush()?;
    let mut replacement = String::new();
    io::stdin().read_line(&mut replacement)?;
    Ok(resolve_accepted_name(generated_name, Some(&replacement)))
}
