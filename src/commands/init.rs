use crate::{
    error::{GlorpError, Result},
    game::{calibration::CalibrationBaseline, evolution::Stage, metabolism::RhythmProfile},
    paths::AppPaths,
    pet::generation::{generate_pet, resolve_accepted_name},
    storage::{
        state::{HabitatState, NarrativeEvent, PetIdentity, PetState, StateStore, Vitals},
        usage_store::UsageStore,
    },
    usage::{agentsview::AgentsviewCommandProvider, provider::UsageProvider},
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
        if let Ok(snapshot) =
            AgentsviewCommandProvider::from_environment().snapshot_for_calibration(&mut usage_store)
        {
            let now = OffsetDateTime::now_utc();
            // Advance cursors unconditionally to prevent a bolus on the next poll.
            // Even when diagnostics are present (e.g. a malformed model breakdown
            // alongside valid records), the cursor_updates reflect current totals
            // and are safe to advance. Skipping this step leaves cursors at zero,
            // so the next clean poll diffs against nothing and applies the full
            // usage history as pet food in one shot.
            usage_store.advance_cursors(snapshot.cursor_updates, now)?;
            if snapshot.diagnostics.is_empty() {
                calibration = CalibrationBaseline::from_history(&snapshot.daily_usage);
                rhythm = RhythmProfile::from_history(&snapshot.daily_usage);
                usage_store.mark_token_contract_active(
                    crate::usage::token_contract::TOKENMAXXING_TOTAL_V1,
                    now,
                )?;
            }
        }
    }

    let now = OffsetDateTime::now_utc();
    let state = PetState {
        schema_version: 1,
        pet: PetIdentity {
            seed,
            generated_species: generated.species,
            accepted_name: accepted_name.clone(),
        },
        stage: Stage::S0,
        xp: 0.0,
        lifetime_effective_tokens: 0.0,
        vitals: Vitals { fed: 70.0, happiness: 70.0, energy: 70.0 },
        calibration,
        rhythm,
        seen_stage_transitions: Vec::new(),
        recent_events: vec![NarrativeEvent {
            observed_at: now,
            text: format!("{accepted_name} has hatched"),
        }],
        habitat: HabitatState::default(),
        reflected_usage_event_ids: Vec::new(),
        last_seen_mood: None,
        previous_vitals: None,
        last_idle_narration_at: None,
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
