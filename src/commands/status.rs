use crate::{
    error::{GlorpError, Result},
    game::evolution::{next_stage_xp_target, stage_start_xp, Stage},
    game::runtime::{apply_unapplied_usage, stage_usage_poll_deltas},
    paths::AppPaths,
    storage::{day_axis::LocalDayMapper, state::StateStore, usage_store::UsageStore},
    usage::{
        agentsview::{needs_full_history_poll, AgentsviewCommandProvider},
        provider::UsageProvider,
    },
};
use time::OffsetDateTime;

pub fn run() -> Result<()> {
    let paths = AppPaths::resolve()?;
    let store = StateStore::new(paths.state_file);
    let Some(mut state) = store.load()? else {
        return Err(GlorpError::Message(
            "no glorp pet exists yet; run `glorp init` first".into(),
        ));
    };

    let mut provider_line = "provider: blocked".to_string();
    let mut provider_health_line = "provider health: blocked".to_string();
    let mut usage_confidence = "snapshot pending".to_string();
    let mut recent_effective = 0.0;
    let mut today_effective = 0.0;
    let mut today_sources: Vec<(String, f64)> = Vec::new();
    let mut diagnostic_line = None;
    if let Ok(mut usage_store) = UsageStore::open(&paths.usage_db) {
        let config = crate::config::AppConfig::load_or_default(&paths.config_file)?;
        let provider = AgentsviewCommandProvider::from_environment();
        let cutover = crate::usage::cutover::ensure_tokenmaxxing_contract_active(
            &mut state,
            &mut usage_store,
            &provider,
            OffsetDateTime::now_utc(),
        )?;
        if cutover.activated {
            store.save(&state)?;
        }
        let poll_result = if !cutover.activated
            && needs_full_history_poll(state.last_usage_poll_at, OffsetDateTime::now_utc())
        {
            provider.poll_full_history(&mut usage_store)
        } else {
            provider.poll(&mut usage_store)
        };
        match poll_result {
            Ok(result) => {
                if !result.deltas.is_empty() || result.diagnostics.is_empty() {
                    let now = OffsetDateTime::now_utc();
                    // Stage smeared ledger rows for new provider deltas before applying.
                    stage_usage_poll_deltas(
                        &mut usage_store,
                        &result,
                        &mut state,
                        config.discontinuity_guard_ratio,
                        now,
                    )?;
                    let mapper = LocalDayMapper::System;
                    let scene_asleep =
                        crate::tui::day::scene_asleep_for_poll(&usage_store, &state, now, mapper);
                    let update =
                        apply_unapplied_usage(&mut state, &mut usage_store, now, scene_asleep)?;
                    store.save(&state)?;
                    // Mark after save: a failure here drifts state.lifetime ahead of the
                    // usage store; the next successful run reconciles via the ledger.
                    usage_store
                        .mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)?;
                    recent_effective = update.recent_effective_tokens;
                }
                if let Some(diagnostic) = result.diagnostics.first() {
                    provider_line = format!("provider: blocked ({})", diagnostic.code);
                    provider_health_line =
                        format!("provider health: blocked ({})", diagnostic.code);
                    diagnostic_line = Some(format!("diagnostic: {}", diagnostic.code));
                } else {
                    provider_line = "provider: local-log-derived".into();
                    provider_health_line = "provider health: ok".into();
                }
                // A refused poll or first-contact seeding persists its
                // diagnostic in the store (the guard writes it during staging);
                // it never rides on result.diagnostics. Surface it without
                // claiming blocked — the source is healthy. Same 1h freshness
                // rule as the watch vm's stale-diagnostic cutoff.
                if diagnostic_line.is_none() {
                    let fresh_cutoff = OffsetDateTime::now_utc() - time::Duration::hours(1);
                    if let Ok(stored) = usage_store.recent_diagnostics(5) {
                        if let Some(refusal) = stored.iter().find(|diagnostic| {
                            (diagnostic.code == crate::game::runtime::USAGE_DISCONTINUITY_CODE
                                || diagnostic.code
                                    == crate::game::runtime::SOURCE_FIRST_CONTACT_CODE)
                                && diagnostic.recorded_at >= fresh_cutoff
                        }) {
                            diagnostic_line = Some(format!(
                                "diagnostic: {} ({})",
                                refusal.code, refusal.provider_surface
                            ));
                        }
                    }
                }
            }
            Err(err) => {
                diagnostic_line = Some(format!("diagnostic: {err}"));
            }
        }

        let status_now = crate::time::usage_now_utc();
        let provider_day = crate::usage::day_axis::tokenmaxxing_provider_day(status_now);
        let snapshot = usage_store.snapshot_totals_for_provider_day(provider_day)?;
        today_effective = snapshot
            .value
            .as_ref()
            .map(|totals| totals.total_tokens)
            .unwrap_or(0.0);
        usage_confidence = match snapshot.state {
            crate::usage::snapshot::SnapshotState::Current => "current provider snapshot".into(),
            crate::usage::snapshot::SnapshotState::Stale => "stale provider snapshot".into(),
            crate::usage::snapshot::SnapshotState::Missing => "snapshot pending".into(),
            crate::usage::snapshot::SnapshotState::Blocked => "snapshot blocked".into(),
        };
        let (today_start, today_end) =
            crate::usage::day_axis::tokenmaxxing_today_window(status_now);
        today_sources = usage_store
            .canonical_total_tokens_by_source_between(today_start, today_end)
            .unwrap_or_default();
    }

    println!(
        "{} / {}",
        state.pet.accepted_name, state.pet.generated_species
    );
    println!("stage: {}  xp: {:.2}", state.stage, state.xp);
    println!("{}", stage_progress_line(state.xp));
    println!(
        "vitals: fed {:.0} happy {:.0} energy {:.0}",
        state.vitals.fed, state.vitals.happiness, state.vitals.energy
    );
    println!(
        "provider today ({usage_confidence}): {:.0}",
        display_tokens(today_effective)
    );
    println!(
        "accepted recent food: {:.0}",
        display_tokens(recent_effective)
    );
    println!(
        "pet lifetime food: {:.0}",
        display_tokens(state.lifetime_effective_tokens)
    );
    if !today_sources.is_empty() {
        println!("sources today:");
        for (name, tokens) in today_sources.iter() {
            println!("  {}: {:.0}", name, display_tokens(*tokens));
        }
    }
    println!("{provider_line}");
    println!("{provider_health_line}");
    println!("cost: local-derived display metadata only");
    if let Some(event) = state.recent_events.last() {
        println!("event: {}", event.text);
    }
    if let Some(line) = diagnostic_line {
        println!("{line}");
    }
    Ok(())
}

fn display_tokens(value: f64) -> f64 {
    value.max(0.0)
}

fn stage_progress_line(xp: f64) -> String {
    let xp = xp.max(0.0);
    let stage = crate::game::evolution::stage_for_xp(xp);
    if matches!(stage, Stage::S6) {
        return "stage progress: s6 complete".into();
    }

    let start = stage_start_xp(stage);
    let next = next_stage_xp_target(stage);
    let span = (next - start).max(f64::EPSILON);
    let percent = ((xp - start) / span * 100.0).clamp(0.0, 100.0);

    format!(
        "stage progress: {:.0}% to {}",
        percent,
        Stage::from_index(stage.index() + 1).unwrap_or(Stage::S6)
    )
}
