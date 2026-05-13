use crate::{
    error::{GlorpError, Result},
    format::format_tokens,
    game::{
        evolution::Stage,
        metabolism::{apply_food, Mood, Vitals as GameVitals},
        runtime::{apply_unapplied_usage, stage_usage_poll_deltas},
    },
    paths::AppPaths,
    pet::{
        art::stage_label,
        generation::generate_pet,
        render::{render_pet, AnimationFrame},
    },
    storage::{
        state::{PetState, StateStore},
        usage_store::{NormalizedUsageEvent, UsageStore},
    },
    tui::{
        app::{WatchApp, WatchUsagePoller},
        style::LogKind,
        view_model::{
            BioView, EventView, PetRenderModel, ProgressView, SourceHealthView, SourceStatus,
            SourceUsageView, WatchViewModel,
        },
    },
    usage::{ccusage::CcusageCommandProvider, provider::UsageProvider},
};
use std::path::Path;
use time::{Duration, OffsetDateTime};

pub fn run() -> Result<()> {
    let paths = AppPaths::resolve()?;
    let state_store = StateStore::new(paths.state_file.clone());
    let Some(state) = state_store.load()? else {
        return Err(GlorpError::Message(
            "no glorp pet exists yet; run `glorp init` first".into(),
        ));
    };

    let vm = build_watch_view_model(&state, &paths.usage_db)?;
    WatchApp::with_poll_callback(
        vm,
        Default::default(),
        Box::new(RealWatchPoller {
            state_file: paths.state_file,
            usage_db: paths.usage_db,
            config_file: paths.config_file,
        }),
    )
    .run()
}

pub fn build_watch_view_model(state: &PetState, usage_db: &Path) -> Result<WatchViewModel> {
    build_watch_view_model_at(state, usage_db, OffsetDateTime::now_utc())
}

pub(crate) fn build_watch_view_model_at(
    state: &PetState,
    usage_db: &Path,
    now: OffsetDateTime,
) -> Result<WatchViewModel> {
    let usage_store = UsageStore::open(usage_db)?;
    let recent_usage = usage_store.recent_events(500)?;
    let species = state.pet.generated_species;
    let stage = state.stage;
    let mood = mood_from_state(state);
    let generated = generate_pet(&state.pet.seed).with_species(species);
    let rendered = render_pet(
        &generated,
        stage,
        mood,
        AnimationFrame {
            tick: now.unix_timestamp().max(0) as u64,
            blink_suppression_ticks: 0,
        },
    );

    // All "today" / "last 10m" framing uses local time. `now` arrives in UTC;
    // here we resolve the user's local offset once and derive every boundary
    // from it so the watch view matches what the user reads on the wall clock.
    let local_offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let now_local = now.to_offset(local_offset);
    let today_start = time::PrimitiveDateTime::new(now_local.date(), time::Time::MIDNIGHT)
        .assume_offset(local_offset);
    let last_10m_start = now - Duration::minutes(10);

    let today_totals = usage_store
        .token_totals_by_source_between(today_start, now)
        .unwrap_or_default();
    let last_10m_totals = usage_store
        .token_totals_by_source_between(last_10m_start, now)
        .unwrap_or_default();
    let today_total_tokens: f64 = today_totals.iter().map(|(_, v)| *v).sum();
    let last_10m_total_tokens: f64 = last_10m_totals.iter().map(|(_, v)| *v).sum();
    let source_breakdown: Vec<SourceUsageView> = today_totals
        .iter()
        .map(|(name, v)| SourceUsageView {
            name: name.clone(),
            effective_tokens: *v,
        })
        .collect();

    // Stale diagnostics shouldn't keep a source marked broken forever.
    // After STALE_DIAGNOSTIC_CUTOFF without a fresh failure, treat the
    // diagnostic as resolved and let the source go back to healthy idle.
    const STALE_DIAGNOSTIC_CUTOFF: Duration = Duration::hours(1);
    let cutoff = now - STALE_DIAGNOSTIC_CUTOFF;
    let all_diagnostics: Vec<_> = usage_store
        .recent_diagnostics(5)?
        .into_iter()
        .filter(|d| d.recorded_at >= cutoff)
        .collect();
    let source_health = source_health(&today_totals, &last_10m_totals, &all_diagnostics);
    let diagnostics = active_diagnostics(&source_breakdown, all_diagnostics);
    let helper_status = helper_status(&usage_store, &source_breakdown, &diagnostics)?;
    let pet_activities = crate::pet::activity::derive_pet_activities(
        &state.pet.accepted_name,
        species,
        mood,
        &recent_usage,
        &state.seen_stage_transitions,
        now,
    );
    let recent_events = build_recent_events(state, &recent_usage, &diagnostics, pet_activities);
    let errors = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect::<Vec<_>>();

    Ok(WatchViewModel {
        pet_art: rendered.lines,
        pet_spans: rendered.spans,
        pet_render: PetRenderModel {
            seed: state.pet.seed.clone(),
            generated_species: state.pet.generated_species,
            stage: state.stage,
            mood,
        },
        pet_name: state.pet.accepted_name.clone(),
        species: species.as_str().to_string(),
        stage: stage_label(species, stage).to_string(),
        mood: mood.as_str().to_string(),
        age_days: (now - state.created_at).whole_days().max(0) as u32,
        xp_current: state.xp,
        xp_target: next_stage_xp_target(stage),
        fed: state.vitals.fed / 100.0,
        happiness: state.vitals.happiness / 100.0,
        energy: state.vitals.energy / 100.0,
        today_effective_tokens: today_total_tokens,
        recent_daily_effective_tokens: usage_store
            .seven_day_token_history(now_local.date())
            .unwrap_or_else(|_| vec![0.0; 7]),
        source_breakdown,
        source_health,
        current_bucket_effective_tokens: last_10m_total_tokens,
        recent_events,
        helper_status,
        errors,
        latest_evolution: state
            .seen_stage_transitions
            .last()
            .map(|stage| stage.as_str().to_string()),
        cursor_screen: None,
        mouse_tracking_enabled: true,
        current_speech: crate::pet::speech::current_pet_speech(
            mood,
            recent_tokens_per_min(&recent_usage, now),
            now,
        ),
        wander_offset_x: crate::pet::animator::compute_wander_offset(now),
        breath_offset_y: crate::pet::animator::compute_breath_offset(Some(species), now),
        progress: {
            let rate_events = usage_store.events_within(Duration::hours(1), now)?;
            let rate_per_hour = progress_rate_per_hour(&rate_events, now);
            let is_max = matches!(stage, Stage::S6);
            let xp_to_next = next_stage_xp_target(stage);
            let xp_in_stage = state.xp;
            let fraction = if xp_to_next <= 0.0 || is_max {
                1.0
            } else {
                (xp_in_stage / xp_to_next).clamp(0.0, 1.0) as f32
            };
            let next_stage_label = if is_max {
                "—".to_string()
            } else {
                let next = match stage {
                    Stage::S0 => Stage::S1,
                    Stage::S1 => Stage::S2,
                    Stage::S2 => Stage::S3,
                    Stage::S3 => Stage::S4,
                    Stage::S4 => Stage::S5,
                    Stage::S5 => Stage::S6,
                    Stage::S6 => Stage::S6,
                };
                stage_label(species, next).to_string()
            };
            ProgressView {
                stage_label: stage_label(species, stage).to_string(),
                next_stage_label,
                fraction,
                xp_in_stage,
                xp_to_next,
                rate_per_hour,
                is_max_stage: is_max,
            }
        },
        bio: {
            let age = now - state.created_at;
            let age_label = BioView::format_age(age);
            let local = state.created_at.to_offset(local_offset);
            let month_name = match local.month() {
                time::Month::January => "jan",
                time::Month::February => "feb",
                time::Month::March => "mar",
                time::Month::April => "apr",
                time::Month::May => "may",
                time::Month::June => "jun",
                time::Month::July => "jul",
                time::Month::August => "aug",
                time::Month::September => "sep",
                time::Month::October => "oct",
                time::Month::November => "nov",
                time::Month::December => "dec",
            };
            let hatched_label = format!(
                "{} {:02} {:02}:{:02}",
                month_name,
                local.day(),
                local.hour(),
                local.minute(),
            );
            BioView {
                hatched_label,
                age_label,
            }
        },
    })
}

/// Tokens observed in the last 60 seconds, returned as a per-minute rate.
fn recent_tokens_per_min(usage_events: &[NormalizedUsageEvent], now: OffsetDateTime) -> f64 {
    let cutoff = now - Duration::minutes(1);
    usage_events
        .iter()
        .filter(|e| e.observed_at >= cutoff)
        .map(|e| e.effective_tokens)
        .sum()
}

#[doc(hidden)]
pub fn build_watch_view_model_for_test(
    state: &PetState,
    usage_db: &Path,
) -> Result<WatchViewModel> {
    build_watch_view_model(state, usage_db)
}

#[doc(hidden)]
pub fn build_watch_view_model_for_test_at(
    state: &PetState,
    usage_db: &Path,
    now: OffsetDateTime,
) -> Result<WatchViewModel> {
    build_watch_view_model_at(state, usage_db, now)
}

struct RealWatchPoller {
    state_file: std::path::PathBuf,
    usage_db: std::path::PathBuf,
    config_file: std::path::PathBuf,
}

impl WatchUsagePoller for RealWatchPoller {
    fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchViewModel> {
        let state_store = StateStore::new(self.state_file.clone());
        let state = match poll_usage_and_apply(&state_store, &self.usage_db, &self.config_file) {
            Ok(Some(state)) => state,
            Ok(None) => {
                return Err(GlorpError::Message(
                    "no glorp pet exists yet; run `glorp init` first".into(),
                ));
            }
            Err(err) => {
                let mut vm = current.clone();
                vm.helper_status = "provider poll failed".into();
                vm.errors.push(err.to_string());
                vm.recent_events.push(EventView {
                    timestamp: timestamp_column(OffsetDateTime::now_utc()),
                    kind: LogKind::Diagnostic,
                    text: err.to_string(),
                });
                return Ok(vm);
            }
        };
        build_watch_view_model(&state, &self.usage_db)
    }
}

fn poll_usage_and_apply(
    state_store: &StateStore,
    usage_db: &Path,
    config_file: &Path,
) -> Result<Option<PetState>> {
    let Some(mut state) = state_store.load()? else {
        return Ok(None);
    };
    let mut usage_store = UsageStore::open(usage_db)?;
    let config = crate::config::AppConfig::load_or_default(config_file)?;
    let weights = crate::game::effective_tokens::EffectiveTokenWeights::from_config(config);
    let result =
        CcusageCommandProvider::from_environment_with_weights(weights).poll(&mut usage_store)?;
    if !result.deltas.is_empty() || result.diagnostics.is_empty() {
        let now = OffsetDateTime::now_utc();
        // Stage smeared ledger rows for new provider deltas before applying.
        stage_usage_poll_deltas(&mut usage_store, &result, state.calibration, now)?;
        let update = apply_unapplied_usage(&mut state, &mut usage_store, now)?;
        state_store.save(&state)?;
        // Mark after save: a failure here drifts state.lifetime ahead of the
        // usage store; the next successful run reconciles via the ledger.
        usage_store.mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)?;
    }
    Ok(Some(state))
}

fn mood_from_state(state: &PetState) -> Mood {
    apply_food(
        GameVitals {
            fed: state.vitals.fed,
            happiness: state.vitals.happiness,
            energy: state.vitals.energy,
        },
        0.0,
        1.0,
    )
    .mood
}

pub fn rerender_pet_for_view_model(vm: &mut WatchViewModel, tick: u64) -> Result<()> {
    let species = vm.pet_render.generated_species;
    let generated = generate_pet(&vm.pet_render.seed).with_species(species);
    let rendered = render_pet(
        &generated,
        vm.pet_render.stage,
        vm.pet_render.mood,
        AnimationFrame {
            tick,
            blink_suppression_ticks: 0,
        },
    );
    vm.pet_art = rendered.lines;
    vm.pet_spans = rendered.spans;
    Ok(())
}

fn next_stage_xp_target(stage: Stage) -> f64 {
    match stage {
        Stage::S0 => 0.04,
        Stage::S1 => 0.25,
        Stage::S2 => 1.0,
        Stage::S3 => 4.0,
        Stage::S4 => 14.0,
        Stage::S5 | Stage::S6 => 60.0,
    }
}

/// Effective tokens observed in the trailing 60 minutes, expressed as a rate
/// in tokens/hour. Simple sum (not an EMA): the display matches the user's
/// mental model — "how much I've burned recently" — and decays to zero one
/// hour after the user actually stops, instead of staying high for many
/// hours after a heavy session ends.
fn progress_rate_per_hour(events: &[NormalizedUsageEvent], now: OffsetDateTime) -> f64 {
    let cutoff = now - Duration::hours(1);
    events
        .iter()
        .filter(|e| e.observed_at >= cutoff)
        .map(|e| e.effective_tokens)
        .sum()
}

fn source_health(
    today_totals: &[(String, f64)],
    last_10m_totals: &[(String, f64)],
    diagnostics: &[crate::storage::usage_store::ProviderDiagnostic],
) -> Vec<SourceHealthView> {
    let mut names = std::collections::BTreeSet::new();
    for (name, _) in today_totals {
        names.insert(name.clone());
    }
    for (name, _) in last_10m_totals {
        names.insert(name.clone());
    }
    for diagnostic in diagnostics {
        names.insert(diagnostic.provider_surface.clone());
    }

    let lookup = |totals: &[(String, f64)], target: &str| -> f64 {
        totals
            .iter()
            .find(|(name, _)| name == target)
            .map(|(_, v)| *v)
            .unwrap_or(0.0)
    };

    names
        .into_iter()
        .map(|name| {
            let today_effective_tokens = lookup(today_totals, &name);
            let bucket_effective_tokens = lookup(last_10m_totals, &name);
            let diagnostic = diagnostics
                .iter()
                .find(|diagnostic| diagnostic.provider_surface == name);
            let status = if today_effective_tokens > 0.0 || bucket_effective_tokens > 0.0 {
                SourceStatus::Ready
            } else if diagnostic.is_some() {
                SourceStatus::Diagnostic
            } else {
                SourceStatus::Blocked
            };
            SourceHealthView {
                name,
                status,
                today_effective_tokens,
                bucket_effective_tokens,
                diagnostic_code: diagnostic.map(|diagnostic| diagnostic.code.clone()),
                diagnostic_message: diagnostic.map(|diagnostic| diagnostic.message.clone()),
            }
        })
        .collect()
}

fn active_diagnostics(
    sources: &[SourceUsageView],
    diagnostics: Vec<crate::storage::usage_store::ProviderDiagnostic>,
) -> Vec<crate::storage::usage_store::ProviderDiagnostic> {
    let ready_today = sources
        .iter()
        .map(|source| source.name.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    diagnostics
        .into_iter()
        .filter(|diagnostic| !ready_today.contains(diagnostic.provider_surface.as_str()))
        .collect()
}

fn build_recent_events(
    state: &PetState,
    usage_events: &[NormalizedUsageEvent],
    diagnostics: &[crate::storage::usage_store::ProviderDiagnostic],
    pet_activities: Vec<EventView>,
) -> Vec<EventView> {
    let mut events = Vec::new();
    for event in state.recent_events.iter().rev().take(3).rev() {
        events.push(EventView {
            timestamp: "--:--".into(),
            kind: LogKind::Normal,
            text: event.clone(),
        });
    }
    for usage_event in aggregated_recent_usage(usage_events, 4) {
        events.push(usage_event);
    }
    for diagnostic_event in deduped_recent_diagnostics(diagnostics, 2) {
        events.push(diagnostic_event);
    }
    // Pet activities are rendered as if they happened "now" — append at the
    // end so they sit at the bottom of the feed (most recent).
    events.extend(pet_activities);
    events
}

/// Group rows that share a `provider_delta_id` so a single smeared real
/// delta surfaces as one log entry. Rows with no `provider_delta_id`
/// stay ungrouped, one entry per row.
fn aggregated_recent_usage(usage_events: &[NormalizedUsageEvent], take: usize) -> Vec<EventView> {
    #[derive(Default)]
    struct Group {
        observed_at: Option<OffsetDateTime>,
        provider_surface: String,
        effective_tokens: f64,
    }

    // Walk newest-first; `recent_events` returns rows ordered DESC by
    // observed_at, but be defensive and record the latest seen per group.
    let mut groups: Vec<Group> = Vec::new();
    let mut group_index_by_id: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for event in usage_events {
        if let Some(delta_id) = event.provider_delta_id.as_deref() {
            if let Some(&index) = group_index_by_id.get(delta_id) {
                let group = &mut groups[index];
                group.effective_tokens += event.effective_tokens;
                if group
                    .observed_at
                    .map(|current| current < event.observed_at)
                    .unwrap_or(true)
                {
                    group.observed_at = Some(event.observed_at);
                }
            } else {
                group_index_by_id.insert(delta_id.to_string(), groups.len());
                groups.push(Group {
                    observed_at: Some(event.observed_at),
                    provider_surface: event.provider_surface.clone(),
                    effective_tokens: event.effective_tokens,
                });
            }
        } else {
            groups.push(Group {
                observed_at: Some(event.observed_at),
                provider_surface: event.provider_surface.clone(),
                effective_tokens: event.effective_tokens,
            });
        }
    }

    groups
        .into_iter()
        .take(take)
        .rev()
        .filter_map(|group| {
            let observed_at = group.observed_at?;
            Some(EventView {
                timestamp: timestamp_column(observed_at),
                kind: LogKind::Usage,
                text: format!(
                    "{} added {} effective tokens",
                    group.provider_surface,
                    format_tokens(group.effective_tokens)
                ),
            })
        })
        .collect()
}

/// Keep one entry per `(provider_surface, code)`, newest first, so a poll
/// loop emitting the same diagnostic does not flood the log.
fn deduped_recent_diagnostics(
    diagnostics: &[crate::storage::usage_store::ProviderDiagnostic],
    take: usize,
) -> Vec<EventView> {
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut keep: Vec<&crate::storage::usage_store::ProviderDiagnostic> = Vec::new();
    for diagnostic in diagnostics {
        let key = (diagnostic.provider_surface.clone(), diagnostic.code.clone());
        if seen.insert(key) {
            keep.push(diagnostic);
            if keep.len() == take {
                break;
            }
        }
    }
    keep.into_iter()
        .rev()
        .map(|diagnostic| EventView {
            timestamp: timestamp_column(diagnostic.recorded_at),
            kind: LogKind::Diagnostic,
            text: format!("{}: {}", diagnostic.provider_surface, diagnostic.code),
        })
        .collect()
}

fn helper_status(
    usage_store: &UsageStore,
    sources: &[SourceUsageView],
    diagnostics: &[crate::storage::usage_store::ProviderDiagnostic],
) -> Result<String> {
    if !sources.is_empty() && !diagnostics.is_empty() {
        let ready = sources
            .iter()
            .map(|source| source.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let diagnostic_sources = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.provider_surface.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        return Ok(format!("{ready} ready; {diagnostic_sources} diagnostic"));
    }

    if !diagnostics.is_empty() {
        let versions = usage_store.provider_versions()?;
        if !versions.is_empty() {
            return Ok(format!(
                "{} ready; usage diagnostic",
                versions
                    .iter()
                    .map(|version| version.provider_surface.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        return Ok("diagnostic from usage helper".into());
    }

    let versions = usage_store.provider_versions()?;
    if versions.is_empty() {
        Ok("waiting for usage helper".into())
    } else {
        Ok(format!(
            "helper ready: {}",
            versions
                .iter()
                .map(|version| version.provider_surface.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

fn timestamp_column(timestamp: OffsetDateTime) -> String {
    format!("{:02}:{:02}", timestamp.hour(), timestamp.minute())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::generation::Species;
    use crate::storage::{state::PetState, usage_store::UsageStore};
    use tempfile::tempdir;
    use time::{Date, Month, PrimitiveDateTime, Time};

    fn sample_event_at_for_test(observed_at: OffsetDateTime, tokens: f64) -> NormalizedUsageEvent {
        NormalizedUsageEvent {
            observed_at,
            bucket_at: observed_at,
            ..NormalizedUsageEvent::for_test_at(observed_at, tokens)
        }
    }

    #[test]
    fn build_watch_view_model_populates_bio_view() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        UsageStore::open(&db_path).unwrap();

        let created_at = PrimitiveDateTime::new(
            Date::from_calendar_date(2026, Month::April, 24).unwrap(),
            Time::from_hms(14, 32, 0).unwrap(),
        )
        .assume_utc();
        let now = created_at + Duration::days(18);

        let mut state = PetState::new_for_test("test", "buddy");
        state.created_at = created_at;

        let vm = build_watch_view_model_at(&state, &db_path, now).unwrap();
        assert_eq!(vm.bio.age_label, "18d");
        assert!(
            vm.bio.hatched_label.contains("apr"),
            "got {}",
            vm.bio.hatched_label
        );
        assert!(
            vm.bio.hatched_label.contains("24"),
            "got {}",
            vm.bio.hatched_label
        );
    }

    #[test]
    fn build_watch_view_model_bio_sub_day_age() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        UsageStore::open(&db_path).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();

        let mut state = PetState::new_for_test("test", "buddy");
        state.created_at = now - Duration::hours(4);

        let vm = build_watch_view_model_at(&state, &db_path, now).unwrap();
        assert_eq!(vm.bio.age_label, "0d 4h");
    }

    #[test]
    fn build_watch_view_model_populates_progress_view() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut store = UsageStore::open(&db_path).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store
            .insert_event(&sample_event_at_for_test(now, 50_000.0))
            .unwrap();
        drop(store);

        let mut state = PetState::new_for_test("test", "Mochi");
        state.pet.generated_species = Species::Fuzz;
        state.stage = Stage::S4;
        state.xp = 8.5; // 61% toward S4 target of 14.0

        let vm = build_watch_view_model_at(&state, &db_path, now).unwrap();
        assert_eq!(vm.progress.stage_label, "fuzz");
        assert_eq!(vm.progress.next_stage_label, "archfuzz");
        assert!(
            vm.progress.fraction > 0.5 && vm.progress.fraction < 0.7,
            "expected fraction in (0.5, 0.7), got {}",
            vm.progress.fraction
        );
        assert!(
            vm.progress.rate_per_hour > 0.0,
            "expected positive rate, got {}",
            vm.progress.rate_per_hour
        );
        assert!(!vm.progress.is_max_stage);
    }

    #[test]
    fn build_watch_view_model_progress_at_s6_is_max_stage() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        UsageStore::open(&db_path).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();

        let mut state = PetState::new_for_test("test", "Mochi");
        state.pet.generated_species = Species::Fuzz;
        state.stage = Stage::S6;
        state.xp = 100.0;

        let vm = build_watch_view_model_at(&state, &db_path, now).unwrap();
        assert!(vm.progress.is_max_stage);
        assert_eq!(vm.progress.next_stage_label, "—");
    }

    #[test]
    fn progress_rate_per_hour_empty_returns_zero() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        assert_eq!(progress_rate_per_hour(&[], now), 0.0);
    }

    #[test]
    fn progress_rate_per_hour_sums_events_in_last_hour() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let recent_a = sample_event_at_for_test(now - Duration::minutes(5), 30_000.0);
        let recent_b = sample_event_at_for_test(now - Duration::minutes(45), 70_000.0);
        let rate = progress_rate_per_hour(&[recent_a, recent_b], now);
        assert_eq!(rate, 100_000.0);
    }

    #[test]
    fn progress_rate_per_hour_excludes_events_older_than_one_hour() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        // Two hours ago is outside the 60-minute window — even though the
        // old EMA would have weighted this at ~80%, the new rate ignores it
        // entirely so the display decays to zero an hour after activity stops.
        let aged = sample_event_at_for_test(now - Duration::hours(2), 1_000_000.0);
        let rate = progress_rate_per_hour(&[aged], now);
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn progress_rate_per_hour_boundary_event_is_included() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let on_boundary = sample_event_at_for_test(now - Duration::hours(1), 12_345.0);
        let rate = progress_rate_per_hour(&[on_boundary], now);
        assert_eq!(rate, 12_345.0);
    }
}
