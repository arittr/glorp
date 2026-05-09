use crate::{
    error::{GlorpError, Result},
    game::{calibration::CalibrationBaseline, metabolism::RhythmProfile},
    paths::AppPaths,
    pet::generation::{generate_pet, resolve_accepted_name},
    storage::{
        state::{PetIdentity, PetState, StateStore, Vitals},
        usage_store::UsageStore,
    },
    usage::{ccusage::CcusageCommandProvider, provider::UsageProvider},
};
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
    let accepted_name = resolve_accepted_name(&generated.generated_name, name.as_deref());

    let mut calibration = CalibrationBaseline::default();
    let mut rhythm = RhythmProfile::default();
    if let Ok(mut usage_store) = UsageStore::open(&paths.usage_db) {
        let _ = CcusageCommandProvider::from_environment().poll(&mut usage_store);
        if let Ok(events) = usage_store.recent_events(90) {
            let history = events
                .iter()
                .map(|event| {
                    crate::game::calibration::DailyUsage::with_activity_timestamp(
                        event.period_start,
                        event.effective_tokens,
                    )
                })
                .collect::<Vec<_>>();
            calibration = CalibrationBaseline::from_history(&history);
            rhythm = RhythmProfile::from_history(&history);
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
