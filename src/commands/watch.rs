use crate::{
    error::{GlorpError, Result},
    format::format_tokens,
    game::{
        evolution::{next_stage_xp_target, stage_start_xp, Stage},
        metabolism::{apply_food, Mood, Vitals as GameVitals},
        runtime::{apply_unapplied_usage, stage_usage_poll_deltas},
    },
    paths::AppPaths,
    pet::{
        art::stage_label,
        generation::{generate_pet, Species},
        render::{render_pet, work_accent_for_profile, AnimationFrame},
    },
    storage::{
        day_axis::LocalDayMapper,
        state::{PetState, StateStore},
        usage_store::{NormalizedUsageEvent, UsageStore},
    },
    tui::{
        app::{WatchApp, WatchPollResult, WatchUsagePoller},
        identity::{
            derive_recovery_pattern, derive_relative_intensity, derive_source_diversity,
            derive_token_shape_personality, derive_work_rhythm, ActivityIdentityProfile,
        },
        style::{source_display_name, LogKind},
        view_model::{
            BioView, EarnedHabitatPropView, EventView, HabitatView, PetRenderModel, ProgressView,
            RateDirection, RateMomentum, RateWindow, SourceHealthView, SourceStatus,
            SourceUsageView, WatchViewModel,
        },
    },
    usage::{
        agentsview::{needs_full_history_poll, AgentsviewCommandProvider},
        provider::UsageProvider,
    },
};
use std::path::Path;
use time::{Duration, OffsetDateTime};

#[cfg(feature = "dev-preview")]
pub fn run(dev_pet: Option<Species>) -> Result<()> {
    run_against_real_state(dev_pet)
}

#[cfg(not(feature = "dev-preview"))]
pub fn run() -> Result<()> {
    run_against_real_state(None)
}

fn run_against_real_state(dev_pet: Option<Species>) -> Result<()> {
    let paths = AppPaths::resolve()?;
    let state_store = StateStore::new(paths.state_file.clone());
    let Some(state) = state_store.load()? else {
        return Err(GlorpError::Message(
            "no glorp pet exists yet; run `glorp init` first".into(),
        ));
    };

    let now = OffsetDateTime::now_utc();
    let mut vm = build_watch_view_model_at(&state, &paths.usage_db, now, LocalDayMapper::System)?;
    if let Some(species) = dev_pet {
        apply_dev_pet_species_override(&mut vm, species, now)?;
    }
    WatchApp::with_poll_callback(
        vm,
        Default::default(),
        Box::new(RealWatchPoller {
            state_file: paths.state_file,
            usage_db: paths.usage_db,
            config_file: paths.config_file,
            dev_pet,
        }),
    )
    .run()
}

#[cfg(feature = "dev-preview")]
pub fn build_dev_pet_view_model(
    state: &PetState,
    usage_db: &Path,
    species: Species,
) -> Result<WatchViewModel> {
    build_dev_pet_view_model_at(
        state,
        usage_db,
        OffsetDateTime::now_utc(),
        LocalDayMapper::System,
        species,
    )
}

#[cfg(feature = "dev-preview")]
pub fn build_dev_pet_view_model_at(
    state: &PetState,
    usage_db: &Path,
    now: OffsetDateTime,
    mapper: LocalDayMapper,
    species: Species,
) -> Result<WatchViewModel> {
    let mut vm = build_watch_view_model_at(state, usage_db, now, mapper)?;
    apply_dev_pet_species_override(&mut vm, species, now)?;
    Ok(vm)
}

pub fn build_watch_view_model(state: &PetState, usage_db: &Path) -> Result<WatchViewModel> {
    build_watch_view_model_at(
        state,
        usage_db,
        OffsetDateTime::now_utc(),
        LocalDayMapper::System,
    )
}

pub(crate) fn build_watch_view_model_at(
    state: &PetState,
    usage_db: &Path,
    now: OffsetDateTime,
    mapper: LocalDayMapper,
) -> Result<WatchViewModel> {
    let usage_store = UsageStore::open(usage_db)?;
    let day_context = crate::tui::day::build_day_context(&usage_store, state, now, mapper);
    let recent_usage = usage_store.recent_events(500)?;
    let species = state.pet.generated_species;
    let stage = state.stage;
    let mood = mood_from_state(state);
    let generated = generate_pet(&state.pet.seed).with_species(species);
    let life_profile = crate::tui::life::PetLifeProfile::default();

    // Keep local time for presentation and pet behavior; visible token totals
    // below use the Tokenmaxxing Los Angeles accounting day.
    let local_offset = mapper.offset_at(now);
    let provider_day = crate::usage::day_axis::tokenmaxxing_provider_day(now);
    let (today_start, _today_end) = crate::usage::day_axis::tokenmaxxing_today_window(now);
    let rate_anchor = normalized_window_anchor(now);
    let window_end = rate_anchor + Duration::seconds(1);
    let last_10m_start = rate_anchor - Duration::minutes(10);
    // Query bounds are half-open. `window_end` nudges the end by one
    // whole second so rows stamped exactly at `now` remain visible without
    // mixing fractional and whole-second RFC3339 bounds.

    let today_snapshot = usage_store.snapshot_totals_for_provider_day(provider_day)?;
    let source_snapshot = usage_store.snapshot_totals_by_source_for_provider_day(provider_day)?;
    let today_total_tokens = today_snapshot
        .value
        .as_ref()
        .map(|totals| totals.total_tokens)
        .unwrap_or(0.0);
    let today_totals = source_snapshot
        .value
        .as_ref()
        .map(|totals| {
            totals
                .sources
                .iter()
                .map(|source| (source.accounting_source.clone(), source.total_tokens))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let last_10m_totals = usage_store
        .canonical_total_tokens_by_source_between(last_10m_start, window_end)
        .unwrap_or_default();
    let snapshot_health =
        usage_store.snapshot_health_for_provider_day(provider_day, &last_10m_totals)?;
    let pulse_window = build_rate_window(&usage_store, rate_anchor, Duration::minutes(10));
    let hour_window = build_rate_window(&usage_store, rate_anchor, Duration::hours(1));
    let rate_momentum = RateMomentum {
        pulse: pulse_window,
        hour: hour_window,
        companion_direction: companion_direction(pulse_window.direction, hour_window.direction),
    };
    let source_diversity = derive_source_diversity(&today_totals);
    let rhythm = derive_work_rhythm(&usage_store, today_start, window_end);
    let today_shape = usage_store
        .applied_token_shape_between(today_start, window_end)
        .unwrap_or_default();
    let token_shape = derive_token_shape_personality(today_shape);
    let relative_intensity = derive_relative_intensity(today_total_tokens, state.calibration);
    let recovery = derive_recovery_pattern(&usage_store, now);
    let activity_identity = ActivityIdentityProfile {
        source_diversity,
        rhythm,
        token_shape,
        relative_intensity,
        recovery,
        long_term_milestones: Vec::new(), // Phase E
    };
    let source_breakdown: Vec<SourceUsageView> = today_totals
        .iter()
        .map(|(name, v)| SourceUsageView {
            name: name.clone(),
            display_name: source_display_name(name).to_string(),
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
    let source_health = source_health(&snapshot_health, &all_diagnostics);
    let diagnostics = active_diagnostics(&source_breakdown, all_diagnostics);
    let helper_status = helper_status(&usage_store, &source_breakdown, &diagnostics)?;
    let pet_activities = crate::pet::activity::derive_pet_activities(
        &state.pet.accepted_name,
        species,
        mood,
        &recent_usage,
        &state.seen_stage_transitions,
        now,
        local_offset,
    );
    let recent_events = build_recent_events(
        state,
        &recent_usage,
        &diagnostics,
        pet_activities,
        local_offset,
    );
    let errors = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect::<Vec<_>>();

    let pet_palette = crate::pet::palette::resolve_pet_palette(species, &generated.traits);

    let mut vm = WatchViewModel {
        pet_art: Vec::new(),
        pet_spans: Vec::new(),
        pet_render: PetRenderModel {
            seed: state.pet.seed.clone(),
            generated_species: state.pet.generated_species,
            stage: state.stage,
            mood,
        },
        pet_palette,
        habitat: build_habitat_view(state),
        life_profile,
        activity_identity,
        day_context,
        pet_name: state.pet.accepted_name.clone(),
        species: species.as_str().to_string(),
        stage: stage_label(species, stage).to_string(),
        mood: mood.as_str().to_string(),
        age_days: (now - state.created_at).whole_days().max(0) as u32,
        fed: state.vitals.fed / 100.0,
        happiness: state.vitals.happiness / 100.0,
        energy: state.vitals.energy / 100.0,
        today_effective_tokens: today_total_tokens,
        today_snapshot_state: today_snapshot.state,
        today_snapshot_reason: today_snapshot.reason.clone(),
        recent_daily_effective_tokens: usage_store
            .snapshot_token_history_for_provider_days(
                &crate::usage::day_axis::tokenmaxxing_days_back(now, 7),
            )
            .map(|history| {
                history
                    .into_iter()
                    .map(|snapshot| {
                        snapshot
                            .value
                            .map(|totals| totals.total_tokens)
                            .unwrap_or(0.0)
                    })
                    .collect()
            })
            .unwrap_or_else(|_| vec![0.0; 7]),
        source_breakdown,
        source_health,
        current_bucket_effective_tokens: pulse_window.current_tokens,
        rate_momentum,
        recent_events,
        helper_status,
        errors,
        latest_evolution: state
            .seen_stage_transitions
            .last()
            .map(|stage| stage.as_str().to_string()),
        cursor_screen: None,
        mouse_tracking_enabled: true,
        current_speech: crate::pet::speech::current_pet_speech_for_scene(
            mood,
            &crate::tui::life::PetLifeProfile::default(),
            &day_context,
            now,
        ),
        wander_offset_x: 0, // computed at render time by the panel from area.width
        breath_offset_y: crate::pet::animator::compute_breath_offset_with_rhythm(
            Some(species),
            now,
            crate::pet::animator::breath_rhythm_for_day(&day_context),
        ),
        facing: 1,                // computed at render time by the panel from area.width
        last_feed_pulse_at: None, // populated by WatchApp when a token spike fires
        progress: {
            let rate_per_hour = hour_window.current_tokens;
            let is_max = matches!(stage, Stage::S6);
            let stage_start = stage_start_xp(stage);
            let xp_in_stage = state.xp - stage_start;
            let xp_to_next = next_stage_xp_target(stage) - stage_start;
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
            BioView { hatched_label, age_label }
        },
    };
    rerender_pet_for_view_model(
        &mut vm,
        now.unix_timestamp().max(0) as u64,
        day_context.asleep,
        now,
    )?;
    Ok(vm)
}

fn normalized_window_anchor(now: OffsetDateTime) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(now.unix_timestamp()).unwrap_or(now)
}

fn build_rate_window(
    usage_store: &UsageStore,
    anchor: OffsetDateTime,
    width: Duration,
) -> RateWindow {
    let current_start = anchor - width;
    let current_end = anchor + Duration::seconds(1);
    let previous_start = current_start - width;
    let current = usage_store
        .canonical_total_tokens_between(current_start, current_end)
        .unwrap_or(0.0);
    let previous = usage_store
        .canonical_total_tokens_between(previous_start, current_start)
        .unwrap_or(0.0);
    RateWindow {
        current_tokens: current,
        previous_tokens: previous,
        direction: rate_direction(current, previous),
    }
}

fn rate_direction(current: f64, previous: f64) -> RateDirection {
    let threshold = 1_000.0_f64.max(previous.max(0.0) * 0.10);
    if current > previous + threshold {
        RateDirection::Up
    } else if current < previous - threshold {
        RateDirection::Down
    } else {
        RateDirection::Neutral
    }
}

fn companion_direction(pulse: RateDirection, hour: RateDirection) -> RateDirection {
    match pulse {
        RateDirection::Up | RateDirection::Down => pulse,
        RateDirection::Neutral => hour,
    }
}

fn build_habitat_view(state: &PetState) -> HabitatView {
    let earned_props = state
        .habitat
        .earned_props
        .iter()
        .filter_map(|earned| {
            let spec = crate::game::habitat::catalog_prop(&earned.id)?;
            Some(EarnedHabitatPropView {
                id: earned.id.clone(),
                earned_at: earned.earned_at,
                kind: spec.kind,
                display_priority: spec.display_priority,
                source: earned.source.clone(),
            })
        })
        .collect();

    HabitatView { earned_props }
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
    build_watch_view_model_at(
        state,
        usage_db,
        now,
        LocalDayMapper::Fixed(time::UtcOffset::UTC),
    )
}

struct RealWatchPoller {
    state_file: std::path::PathBuf,
    usage_db: std::path::PathBuf,
    config_file: std::path::PathBuf,
    dev_pet: Option<Species>,
}

impl WatchUsagePoller for RealWatchPoller {
    fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchPollResult> {
        let state_store = StateStore::new(self.state_file.clone());
        let outcome = match poll_usage_and_apply(&state_store, &self.usage_db, &self.config_file) {
            Ok(Some(outcome)) => outcome,
            Ok(None) => {
                return Err(GlorpError::Message(
                    "no glorp pet exists yet; run `glorp init` first".into(),
                ));
            }
            Err(err) => {
                let mut vm = current.clone();
                vm.helper_status = "provider poll failed".into();
                vm.errors.push(err.to_string());
                let now = OffsetDateTime::now_utc();
                vm.recent_events.push(EventView {
                    timestamp: crate::pet::activity::format_hhmm_local(
                        now,
                        LocalDayMapper::System.offset_at(now),
                    ),
                    kind: LogKind::Diagnostic,
                    text: err.to_string(),
                });
                return Ok(WatchPollResult {
                    vm,
                    applied_signal: crate::tui::life::AppliedUsageSignal::diagnostics_only(
                        now,
                        Duration::seconds(0),
                    ),
                });
            }
        };
        let now = OffsetDateTime::now_utc();
        let mut vm =
            build_watch_view_model_at(&outcome.state, &self.usage_db, now, LocalDayMapper::System)?;
        if let Some(species) = self.dev_pet {
            apply_dev_pet_species_override(&mut vm, species, now)?;
        }
        Ok(WatchPollResult {
            vm,
            applied_signal: outcome.applied_signal,
        })
    }
}

pub(crate) struct PollUsageOutcome {
    pub state: PetState,
    pub applied_signal: crate::tui::life::AppliedUsageSignal,
}

pub(crate) fn poll_usage_and_apply(
    state_store: &StateStore,
    usage_db: &Path,
    config_file: &Path,
) -> Result<Option<PollUsageOutcome>> {
    let Some(mut state) = state_store.load()? else {
        return Ok(None);
    };
    let mut usage_store = UsageStore::open(usage_db)?;
    let config = crate::config::AppConfig::load_or_default(config_file)?;
    let now = OffsetDateTime::now_utc();
    let provider = AgentsviewCommandProvider::from_environment();
    let cutover = crate::usage::cutover::ensure_tokenmaxxing_contract_active(
        &mut state,
        &mut usage_store,
        &provider,
        now,
    )?;
    if cutover.activated {
        state_store.save(&state)?;
    }
    let result = if !cutover.activated && needs_full_history_poll(state.last_usage_poll_at, now) {
        provider.poll_full_history(&mut usage_store)?
    } else {
        provider.poll(&mut usage_store)?
    };
    if !result.deltas.is_empty() || result.diagnostics.is_empty() {
        // Stage smeared ledger rows for new provider deltas before applying.
        stage_usage_poll_deltas(
            &mut usage_store,
            &result,
            &mut state,
            config.discontinuity_guard_ratio,
            now,
        )?;
        let scene_asleep = crate::tui::day::scene_asleep_for_poll(
            &usage_store,
            &state,
            now,
            LocalDayMapper::System,
        );
        let update = apply_unapplied_usage(&mut state, &mut usage_store, now, scene_asleep)?;
        let applied_signal = update.applied_signal;
        state_store.save(&state)?;
        // Mark after save: a failure here drifts state.lifetime ahead of the
        // usage store; the next successful run reconciles via the ledger.
        usage_store.mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)?;
        return Ok(Some(PollUsageOutcome { state, applied_signal }));
    }
    let elapsed = state
        .last_usage_poll_at
        .map(|last| now - last)
        .unwrap_or_else(|| Duration::seconds(0));
    Ok(Some(PollUsageOutcome {
        state,
        applied_signal: crate::tui::life::AppliedUsageSignal::diagnostics_only(now, elapsed),
    }))
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

/// Glitch-species persistent corruption inputs for the current view model, or
/// `None` for non-Glitch species. `feed_reaction` mirrors the same token-pop
/// recency check used for the face's `AnimationFrame.feed_reaction`; callers
/// compute it once and pass it in rather than recomputing here.
fn glitch_corruption_frame_for_view_model(
    vm: &WatchViewModel,
    feed_reaction: bool,
) -> Option<crate::pet::render::GlitchCorruptionFrame> {
    if vm.pet_render.generated_species != Species::Glitch {
        return None;
    }
    Some(crate::pet::render::glitch_corruption_frame_for_inputs(
        vm.day_context.date_seed,
        vm.day_context.today_ratio,
        vm.life_profile.burst_level,
        vm.life_profile.calm_mode,
        feed_reaction,
    ))
}

pub fn rerender_pet_for_view_model(
    vm: &mut WatchViewModel,
    tick: u64,
    hold_eyes_closed: bool,
    now: time::OffsetDateTime,
) -> Result<()> {
    // This is the single canonical pet render for the view model: it applies
    // mood -> eye color (riding the same cadence as the eye glyph in
    // expression_for, overwriting only the eye role) and glitch corruption on
    // top of the base species/stage art.
    crate::pet::palette::apply_mood_eye_color(&mut vm.pet_palette, vm.pet_render.mood);
    let species = vm.pet_render.generated_species;
    let generated = generate_pet(&vm.pet_render.seed).with_species(species);
    let pet_performance = crate::tui::room::pet_performance_from_day_context(&vm.day_context);
    let feed_reaction =
        crate::pet::animator::compute_token_pop(vm.last_feed_pulse_at, now).is_some();
    let rendered = render_pet(
        &generated,
        vm.pet_render.stage,
        vm.pet_render.mood,
        AnimationFrame {
            tick,
            hold_eyes_closed,
            blink_slowdown: crate::pet::render::blink_slowdown_for_tiredness(
                vm.day_context.tiredness,
            ),
            soft_eyes: matches!(
                pet_performance,
                crate::tui::room::PetPerformance::TiredAwake
                    | crate::tui::room::PetPerformance::HeavyDayCozy
            ),
            work_accent: work_accent_for_profile(&vm.life_profile),
            feed_reaction,
            glitch_corruption: glitch_corruption_frame_for_view_model(vm, feed_reaction),
            ..AnimationFrame::default()
        },
    );
    vm.pet_art = rendered.lines;
    vm.pet_spans = rendered.spans;
    Ok(())
}

fn apply_dev_pet_species_override(
    vm: &mut WatchViewModel,
    species: Species,
    now: OffsetDateTime,
) -> Result<()> {
    vm.pet_render.generated_species = species;
    let generated = generate_pet(&vm.pet_render.seed).with_species(species);
    vm.pet_palette = crate::pet::palette::resolve_pet_palette(species, &generated.traits);
    vm.species = species.as_str().to_string();
    vm.stage = stage_label(species, vm.pet_render.stage).to_string();
    vm.progress.stage_label = stage_label(species, vm.pet_render.stage).to_string();
    vm.progress.next_stage_label = if vm.progress.is_max_stage {
        "—".to_string()
    } else {
        let next = Stage::from_index((vm.pet_render.stage.index() + 1).min(Stage::S6.index()))
            .unwrap_or(Stage::S6);
        stage_label(species, next).to_string()
    };
    vm.breath_offset_y = crate::pet::animator::compute_breath_offset_with_rhythm(
        Some(species),
        now,
        crate::pet::animator::breath_rhythm_for_day(&vm.day_context),
    );
    rerender_pet_for_view_model(
        vm,
        now.unix_timestamp().max(0) as u64,
        vm.day_context.asleep,
        now,
    )
}

fn source_health(
    snapshot_health: &[crate::usage::snapshot::SourceSnapshotHealth],
    diagnostics: &[crate::storage::usage_store::ProviderDiagnostic],
) -> Vec<SourceHealthView> {
    let mut names = std::collections::BTreeSet::new();
    for source in snapshot_health {
        names.insert(source.accounting_source.clone());
    }
    for diagnostic in diagnostics {
        if diagnostic.code == crate::game::runtime::USAGE_DISCONTINUITY_CODE {
            continue; // a refused poll is not a broken source
        }
        names.insert(diagnostic.provider_surface.clone());
    }

    let lookup = |target: &str| -> Option<&crate::usage::snapshot::SourceSnapshotHealth> {
        snapshot_health
            .iter()
            .find(|source| source.accounting_source == target)
    };

    names
        .into_iter()
        .map(|name| {
            let snapshot = lookup(&name);
            let today_for_source = snapshot
                .and_then(|source| source.snapshot_total_tokens)
                .unwrap_or(0.0);
            let bucket_effective_tokens = snapshot
                .map(|source| source.recent_accepted_tokens)
                .unwrap_or(0.0);
            let diagnostic = diagnostics.iter().find(|diagnostic| {
                diagnostic.provider_surface == name
                    && diagnostic.code != crate::game::runtime::USAGE_DISCONTINUITY_CODE
            });
            let status = if today_for_source > 0.0 || bucket_effective_tokens > 0.0 {
                SourceStatus::Ready
            } else if diagnostic.is_some() {
                SourceStatus::Diagnostic
            } else {
                SourceStatus::Blocked
            };
            SourceHealthView {
                name: name.clone(),
                display_name: source_display_name(&name).to_string(),
                status,
                snapshot_state: snapshot
                    .map(|source| source.snapshot_state)
                    .unwrap_or(crate::usage::snapshot::SnapshotState::Missing),
                snapshot_reason: snapshot.and_then(|source| source.reason.clone()),
                today_effective_tokens: today_for_source,
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
        .filter(|diagnostic| {
            diagnostic.code == crate::game::runtime::USAGE_DISCONTINUITY_CODE
                || !ready_today.contains(diagnostic.provider_surface.as_str())
        })
        .collect()
}

fn build_recent_events(
    state: &PetState,
    usage_events: &[NormalizedUsageEvent],
    diagnostics: &[crate::storage::usage_store::ProviderDiagnostic],
    pet_activities: Vec<EventView>,
    local_offset: time::UtcOffset,
) -> Vec<EventView> {
    // Merge narrative and usage events by observed_at so the feed reads as a
    // single chronological timeline (oldest at top, newest at bottom).
    struct Timestamped {
        observed_at: OffsetDateTime,
        view: EventView,
    }

    let mut merged: Vec<Timestamped> = Vec::new();

    for event in state.recent_events.iter().rev().take(3).rev() {
        // UNIX_EPOCH is the sentinel for legacy entries — keep showing "--:--".
        let timestamp = if event.observed_at == time::OffsetDateTime::UNIX_EPOCH {
            "--:--".into()
        } else {
            crate::pet::activity::format_hhmm_local(event.observed_at, local_offset)
        };
        merged.push(Timestamped {
            observed_at: event.observed_at,
            view: EventView {
                timestamp,
                kind: LogKind::Narrative,
                text: event.text.clone(),
            },
        });
    }

    for (observed_at, view) in aggregated_recent_usage_with_time(usage_events, 4, local_offset) {
        merged.push(Timestamped { observed_at, view });
    }

    merged.sort_by_key(|m| m.observed_at);

    let mut events: Vec<EventView> = merged.into_iter().map(|m| m.view).collect();

    for diagnostic_event in deduped_recent_diagnostics(diagnostics, 2, local_offset) {
        events.push(diagnostic_event);
    }
    // Pet activities are rendered as if they happened "now" — append at the
    // end so they sit at the bottom of the feed (most recent).
    events.extend(pet_activities);
    events
}

/// Group rows that share a `provider_delta_id` so a single smeared real
/// delta surfaces as one log entry. Rows with no `provider_delta_id`
/// stay ungrouped, one entry per row. Returns `(observed_at, EventView)` pairs
/// so callers can merge by timestamp before rendering.
fn aggregated_recent_usage_with_time(
    usage_events: &[NormalizedUsageEvent],
    take: usize,
    local_offset: time::UtcOffset,
) -> Vec<(OffsetDateTime, EventView)> {
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
        .filter_map(|group| {
            let observed_at = group.observed_at?;
            Some((
                observed_at,
                EventView {
                    timestamp: crate::pet::activity::format_hhmm_local(observed_at, local_offset),
                    kind: LogKind::Usage,
                    text: format!(
                        "{} added {} effective tokens",
                        group.provider_surface,
                        format_tokens(group.effective_tokens)
                    ),
                },
            ))
        })
        .collect()
}

/// Keep one entry per `(provider_surface, code)`, newest first, so a poll
/// loop emitting the same diagnostic does not flood the log.
fn deduped_recent_diagnostics(
    diagnostics: &[crate::storage::usage_store::ProviderDiagnostic],
    take: usize,
    local_offset: time::UtcOffset,
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
            timestamp: crate::pet::activity::format_hhmm_local(
                diagnostic.recorded_at,
                local_offset,
            ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::generation::Species;
    use crate::storage::{
        day_axis::LocalDayMapper,
        state::PetState,
        usage_store::{ProviderCursorUpdate, UsageStore},
    };
    use crate::tui::identity::SourceDiversity;
    use crate::usage::identity::SourceIdentity;
    use crate::usage::provider::{UsageDelta, UsagePollResult};
    use tempfile::tempdir;
    use time::{Date, Month, PrimitiveDateTime, Time};

    fn sample_event_at_for_test(observed_at: OffsetDateTime, tokens: f64) -> NormalizedUsageEvent {
        NormalizedUsageEvent {
            observed_at,
            bucket_at: observed_at,
            ..NormalizedUsageEvent::for_test_at(observed_at, tokens)
        }
    }

    fn establish_contact_for_test(
        usage_store: &mut UsageStore,
        surface: &str,
        now: OffsetDateTime,
    ) {
        usage_store
            .advance_cursors(
                vec![ProviderCursorUpdate {
                    provider_surface: surface.to_string(),
                    cursor_key: "catchup-test".to_string(),
                    cursor_value: "seeded".to_string(),
                    provider_version: "test-provider".to_string(),
                    parser_version: "test-parser".to_string(),
                }],
                now,
            )
            .unwrap();
    }

    fn seed_snapshot_for_test(
        usage_store: &mut UsageStore,
        day: Date,
        source: &str,
        total_tokens: f64,
        observed_at: OffsetDateTime,
    ) {
        let batch = crate::usage::snapshot::ProviderSnapshotBatchInput {
            collector_scope_id: format!("{source}:local-usage"),
            collector_surface: format!("ccusage:{source}"),
            command: "test snapshot".into(),
            token_contract: crate::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
            requested_provider_days: vec![day],
            covered_accounting_sources: Some(vec![source.to_string()]),
            provider_version: "test-provider".into(),
            parser_version: "test-parser".into(),
            observed_at,
        };
        let row = crate::usage::snapshot::ProviderSnapshotRowInput {
            replacement_scope_id: format!("{source}:local-usage"),
            collector_scope_id: format!("{source}:local-usage"),
            collector_surface: format!("ccusage:{source}"),
            command: "test snapshot".into(),
            token_contract: crate::usage::token_contract::TOKENMAXXING_TOTAL_V1.into(),
            accounting_source: source.into(),
            provider_day: day,
            model: Some("test-model".into()),
            source_surface: "daily".into(),
            provider_period: day.to_string(),
            raw_source_id_hash: Some(format!("hash:{source}:test")),
            cursor_key_hash: format!("hash:{source}:cursor"),
            cursor_update: ProviderCursorUpdate {
                provider_surface: source.into(),
                cursor_key: "cursor".into(),
                cursor_value: "value".into(),
                provider_version: "test-provider".into(),
                parser_version: "test-parser".into(),
            },
            raw_token_buckets: None,
            total_tokens,
            cost_usd: None,
            confidence: "local-log-derived".into(),
        };
        usage_store
            .write_provider_snapshot_batch(&batch, &[row], &[])
            .unwrap();
    }

    #[cfg(feature = "dev-preview")]
    #[test]
    fn build_dev_pet_view_model_preserves_real_watch_context() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut usage_store = UsageStore::open(&db_path).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        usage_store
            .insert_event(&NormalizedUsageEvent {
                provider_surface: "codex".into(),
                observed_at: now,
                bucket_at: now,
                effective_tokens: 4_200.0,
                ..NormalizedUsageEvent::for_test_at(now, 4_200.0)
            })
            .unwrap();
        seed_snapshot_for_test(
            &mut usage_store,
            crate::usage::day_axis::tokenmaxxing_provider_day(now),
            "codex",
            4_200.0,
            now,
        );

        let mut state = PetState::new_for_test("real-seed", "buddy");
        state.pet.generated_species = Species::Mech;
        state.stage = Stage::S4;
        state.vitals.fed = 88.0;
        state.vitals.happiness = 82.0;
        state.vitals.energy = 61.0;
        state.habitat.earned_props = vec![crate::storage::state::EarnedHabitatProp {
            id: crate::storage::state::HabitatPropId::new(crate::game::habitat::CODEX_SIGNAL_LAMP),
            earned_at: now,
            source: crate::storage::state::HabitatPropSource::ProviderFirstUse {
                provider_surface: "codex".into(),
            },
        }];

        let vm = build_dev_pet_view_model_at(
            &state,
            &db_path,
            now,
            LocalDayMapper::Fixed(time::UtcOffset::UTC),
            Species::Ghost,
        )
        .unwrap();

        assert_eq!(state.pet.generated_species, Species::Mech);
        assert_eq!(vm.pet_render.generated_species, Species::Ghost);
        assert_eq!(vm.species, Species::Ghost.as_str());
        assert_eq!(vm.stage, stage_label(Species::Ghost, Stage::S4));
        let ghost_pet = generate_pet(&state.pet.seed).with_species(Species::Ghost);
        let raw_palette =
            crate::pet::palette::resolve_pet_palette(Species::Ghost, &ghost_pet.traits);
        // Non-eye roles come from the species palette; eye is mood-colored by the
        // per-tick hook in rerender_pet_for_view_model.
        assert_eq!(vm.pet_palette.body, raw_palette.body);
        assert_eq!(vm.pet_palette.mouth, raw_palette.mouth);
        assert_eq!(vm.pet_palette.accent, raw_palette.accent);
        assert_eq!(vm.pet_palette.pattern, raw_palette.pattern);
        assert_eq!(vm.pet_palette.particle, raw_palette.particle);
        assert_eq!(
            vm.pet_palette.eye,
            crate::pet::palette::eye_color_for_mood(vm.pet_render.mood)
        );
        assert_eq!(
            vm.today_snapshot_state,
            crate::usage::snapshot::SnapshotState::Current
        );
        assert_eq!(vm.today_effective_tokens, 4_200.0);
        assert_eq!(vm.source_breakdown.len(), 1);
        assert_eq!(vm.source_breakdown[0].name, "codex");
        assert_eq!(vm.source_breakdown[0].effective_tokens, 4_200.0);
        assert_eq!(vm.habitat.earned_props.len(), 1);
        assert_eq!(vm.habitat.earned_props[0].id.as_str(), "codex_signal_lamp");
        assert_eq!(vm.fed, 0.88);
        assert_eq!(vm.happiness, 0.82);
        assert_eq!(vm.energy, 0.61);
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

        let vm = build_watch_view_model_at(
            &state,
            &db_path,
            now,
            LocalDayMapper::Fixed(time::UtcOffset::UTC),
        )
        .unwrap();
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

        let vm = build_watch_view_model_at(
            &state,
            &db_path,
            now,
            LocalDayMapper::Fixed(time::UtcOffset::UTC),
        )
        .unwrap();
        assert_eq!(vm.bio.age_label, "0d 4h");
    }

    #[test]
    fn build_watch_view_model_populates_progress_view() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut store = UsageStore::open(&db_path).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        store
            .insert_event(&sample_event_at_for_test(
                now - Duration::minutes(1),
                50_000.0,
            ))
            .unwrap();
        drop(store);

        let mut state = PetState::new_for_test("test", "Mochi");
        state.pet.generated_species = Species::Fuzz;
        state.stage = Stage::S4;
        state.xp = 8.5; // S4 spans 4.0..14.0; 8.5 is 4.5/10.0 = 45% through the stage

        let vm = build_watch_view_model_at(
            &state,
            &db_path,
            now,
            LocalDayMapper::Fixed(time::UtcOffset::UTC),
        )
        .unwrap();
        assert_eq!(vm.progress.stage_label, "fuzz");
        assert_eq!(vm.progress.next_stage_label, "archfuzz");
        assert!(
            (vm.progress.fraction - 0.45).abs() < 0.01,
            "expected stage-relative fraction ~0.45, got {}",
            vm.progress.fraction
        );
        assert!((vm.progress.xp_in_stage - 4.5).abs() < 1e-6);
        assert!((vm.progress.xp_to_next - 10.0).abs() < 1e-6);
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

        let vm = build_watch_view_model_at(
            &state,
            &db_path,
            now,
            LocalDayMapper::Fixed(time::UtcOffset::UTC),
        )
        .unwrap();
        assert!(vm.progress.is_max_stage);
        assert_eq!(vm.progress.next_stage_label, "—");
    }

    #[test]
    fn watch_progress_rate_attributes_to_bucket_at_not_observed_at() {
        // Catchup smear: a delta polled "now" smears bucket_at back over 110
        // minutes. The rate must reflect when the activity actually happened
        // (bucket_at), not when the helper noticed it (observed_at) —
        // otherwise a fat trailing report inflates the rate for an hour even
        // though no new tokens are being burned.
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut store = UsageStore::open(&db_path).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let catchup_observed_now_but_old_activity = NormalizedUsageEvent {
            observed_at: now,
            bucket_at: now - Duration::hours(3),
            effective_tokens: 1_000_000.0,
            ..NormalizedUsageEvent::for_test_at(now - Duration::hours(3), 1_000_000.0)
        };
        let recent = NormalizedUsageEvent {
            observed_at: now,
            bucket_at: now - Duration::minutes(15),
            effective_tokens: 42_000.0,
            ..NormalizedUsageEvent::for_test_at(now - Duration::minutes(15), 42_000.0)
        };
        store
            .insert_event(&catchup_observed_now_but_old_activity)
            .unwrap();
        store.insert_event(&recent).unwrap();
        drop(store);

        let mut state = PetState::new_for_test("test", "Mochi");
        state.pet.generated_species = Species::Fuzz;
        state.stage = Stage::S4;
        state.xp = 5.0;
        let vm = build_watch_view_model_at(
            &state,
            &db_path,
            now,
            LocalDayMapper::Fixed(time::UtcOffset::UTC),
        )
        .unwrap();
        assert_eq!(
            vm.progress.rate_per_hour, 42_000.0,
            "catchup row with bucket_at 3h ago must NOT contribute to the rate"
        );
    }

    #[test]
    fn build_recent_events_interleaves_narrative_and_usage_by_timestamp() {
        use crate::storage::state::NarrativeEvent;

        let base = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        // narrative at T+0 and T+20m; usage at T+10m and T+30m
        let t0 = base;
        let t10 = base + Duration::minutes(10);
        let t20 = base + Duration::minutes(20);
        let t30 = base + Duration::minutes(30);

        let mut state = PetState::new_for_test("test", "Buddy");
        state.recent_events = vec![
            NarrativeEvent {
                observed_at: t0,
                text: "Buddy brightened".into(),
            },
            NarrativeEvent {
                observed_at: t20,
                text: "Buddy munched".into(),
            },
        ];

        // Two usage events that interleave with the narrative entries.
        let usage_events = vec![
            NormalizedUsageEvent {
                provider_delta_id: Some("delta-a".into()),
                ..NormalizedUsageEvent::for_test_at(t10, 5_000.0)
            },
            NormalizedUsageEvent {
                provider_delta_id: Some("delta-b".into()),
                ..NormalizedUsageEvent::for_test_at(t30, 8_000.0)
            },
        ];

        let events = build_recent_events(&state, &usage_events, &[], vec![], time::UtcOffset::UTC);

        // Verify chronological order: T+0, T+10, T+20, T+30
        assert_eq!(events.len(), 4);
        assert!(
            events[0].text.contains("brightened"),
            "first: {}",
            events[0].text
        );
        assert!(
            events[1].text.contains("5.0k"),
            "second should be usage 5k: {}",
            events[1].text
        );
        assert!(
            events[2].text.contains("munched"),
            "third: {}",
            events[2].text
        );
        assert!(
            events[3].text.contains("8.0k"),
            "fourth should be usage 8k: {}",
            events[3].text
        );
    }

    fn catchup_poll_result_for_test_n(n: u64, effective_tokens: f64) -> UsagePollResult {
        UsagePollResult {
            deltas: vec![UsageDelta {
                provider_surface: "claude-code".into(),
                source_identity: SourceIdentity::claude_code(),
                command: "ccusage daily --json --offline".into(),
                effective_tokens,
                total_tokens: effective_tokens,
                token_contract: crate::usage::token_contract::WEIGHTED_EFFECTIVE_V1.to_string(),
                confidence: "local-log-derived".into(),
                period_start: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
                observed_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
                model: Some("test-model".into()),
                cursor_update: ProviderCursorUpdate {
                    provider_surface: "claude-code".into(),
                    cursor_key: "catchup-test".into(),
                    cursor_value: format!("cursor-{n}"),
                    provider_version: "test-provider".into(),
                    parser_version: "test-parser".into(),
                },
                token_totals: None,
                feed_highwater_updates: Vec::new(),
            }],
            diagnostics: vec![],
            total_effective_tokens: effective_tokens,
            total_tokens: effective_tokens,
        }
    }

    fn catchup_poll_result_for_test(effective_tokens: f64) -> UsagePollResult {
        UsagePollResult {
            deltas: vec![UsageDelta {
                provider_surface: "claude-code".into(),
                source_identity: SourceIdentity::claude_code(),
                command: "ccusage daily --json --offline".into(),
                effective_tokens,
                total_tokens: effective_tokens,
                token_contract: crate::usage::token_contract::WEIGHTED_EFFECTIVE_V1.to_string(),
                confidence: "local-log-derived".into(),
                period_start: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
                observed_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
                model: Some("test-model".into()),
                cursor_update: ProviderCursorUpdate {
                    provider_surface: "claude-code".into(),
                    cursor_key: "catchup-test".into(),
                    cursor_value: "delayed-row".into(),
                    provider_version: "test-provider".into(),
                    parser_version: "test-parser".into(),
                },
                token_totals: None,
                feed_highwater_updates: Vec::new(),
            }],
            diagnostics: vec![],
            total_effective_tokens: effective_tokens,
            total_tokens: effective_tokens,
        }
    }

    #[test]
    fn status_today_and_watch_today_agree_across_a_midnight_boundary() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut usage = UsageStore::open(&db_path).unwrap();
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::from_hms(-8, 0, 0).unwrap());
        // 23:30 local June 8 (07:30 UTC June 9) — late-night work.
        let late = PrimitiveDateTime::new(
            Date::from_calendar_date(2026, Month::June, 9).unwrap(),
            Time::from_hms(7, 30, 0).unwrap(),
        )
        .assume_utc();
        usage
            .insert_event(&NormalizedUsageEvent {
                observed_at: late,
                bucket_at: late,
                ..NormalizedUsageEvent::for_test_at(late, 3_000.0)
            })
            .unwrap();
        // Now = 00:30 in the fixed local mapper, but Tokenmaxxing's Los Angeles
        // day is on DST in June and starts at 07:00 UTC, so the 07:30 UTC row is
        // part of today's canonical Tokenmaxxing total.
        let now = late + Duration::hours(1);
        let provider_day = crate::usage::day_axis::tokenmaxxing_provider_day(now);
        seed_snapshot_for_test(&mut usage, provider_day, "claude-code", 4_500.0, now);
        let state = PetState::new_for_test("test", "buddy");
        let vm = build_watch_view_model_at(&state, &db_path, now, mapper).unwrap();
        let (today_start, today_end) = crate::usage::day_axis::tokenmaxxing_today_window(now);
        let canonical_today = usage
            .canonical_total_tokens_between(today_start, today_end)
            .unwrap();
        assert_eq!(
            vm.today_snapshot_state,
            crate::usage::snapshot::SnapshotState::Current
        );
        assert_eq!(vm.today_effective_tokens, 4_500.0);
        assert_eq!(canonical_today, 3_000.0);
    }

    #[test]
    fn oversized_staged_backlog_becomes_visible_over_successive_applies() {
        use crate::game::runtime::{apply_unapplied_usage, stage_usage_poll_deltas};
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut usage = UsageStore::open(&db_path).unwrap();
        let mut state = PetState::new_for_test("seed", "buddy");
        state.calibration.daily_effective_tokens = 1_000.0; // force many small buckets
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::UTC);
        // Seed contact so the guard does not refuse every first-contact delta.
        establish_contact_for_test(&mut usage, "claude-code", now);
        // Stage > 500 rows via the REAL path: 60 deltas x ~6-12 smear buckets.
        for i in 0..60 {
            let poll = catchup_poll_result_for_test_n(i, 50_000.0); // distinct cursor per delta
            stage_usage_poll_deltas(
                &mut usage,
                &poll,
                &mut state,
                crate::game::runtime::DISCONTINUITY_GUARD_RATIO,
                now,
            )
            .unwrap();
        }
        let before = usage.today_effective_tokens(now, mapper).unwrap();
        let update = apply_unapplied_usage(&mut state, &mut usage, now, false).unwrap();
        usage
            .mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)
            .unwrap();
        let after_one = usage.today_effective_tokens(now, mapper).unwrap();
        assert_eq!(
            before, 0.0,
            "staged rows are invisible to applied-only reads"
        );
        assert!(after_one > 0.0, "the first apply makes <=500 rows visible");
        // Drain the backlog with successive apply/mark cycles; totals converge.
        for _ in 0..20 {
            let u = apply_unapplied_usage(&mut state, &mut usage, now, false).unwrap();
            usage
                .mark_events_applied_and_advance_cursors(&u.applied_event_ids, now)
                .unwrap();
        }
        let drained = usage.today_effective_tokens(now, mapper).unwrap();
        assert!(
            drained > after_one,
            "successive polls converge the backlog into the visible total"
        );
    }

    #[test]
    fn cold_start_catchup_wakes_the_pet_once_through_the_real_smear_path() {
        use crate::game::runtime::stage_usage_poll_deltas;
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut usage = UsageStore::open(&db_path).unwrap();
        let mut state = PetState::new_for_test("seed", "buddy");
        let now = PrimitiveDateTime::new(
            Date::from_calendar_date(2026, Month::June, 9).unwrap(),
            Time::from_hms(23, 30, 0).unwrap(),
        )
        .assume_utc();
        state.created_at = now - Duration::days(3);
        state.last_usage_poll_at = Some(now - Duration::hours(6)); // long gap => Backfill
                                                                   // Give the pet sleep-eligible history: one applied row hours ago.
        usage
            .insert_event(&NormalizedUsageEvent {
                observed_at: now - Duration::hours(5),
                bucket_at: now - Duration::hours(5),
                ..NormalizedUsageEvent::for_test_at(now - Duration::hours(5), 1_000.0)
            })
            .unwrap();
        let mapper = crate::storage::day_axis::LocalDayMapper::Fixed(time::UtcOffset::UTC);
        let pre = crate::tui::day::build_day_context(&usage, &state, now, mapper);
        assert!(pre.asleep, "pet is asleep before the catch-up poll");

        // Seed contact so the guard does not refuse the catch-up delta.
        establish_contact_for_test(&mut usage, "claude-code", now);
        // Drive the REAL smear: a poll result with one fat 6h-old delta.
        let poll = catchup_poll_result_for_test(120_000.0);
        stage_usage_poll_deltas(
            &mut usage,
            &poll,
            &mut state,
            crate::game::runtime::DISCONTINUITY_GUARD_RATIO,
            now,
        )
        .unwrap();
        let update =
            crate::game::runtime::apply_unapplied_usage(&mut state, &mut usage, now, false)
                .unwrap();
        usage
            .mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)
            .unwrap();

        let post = crate::tui::day::build_day_context(&usage, &state, now, mapper);
        assert!(
            !post.asleep,
            "the accepted catch-up wake: newly applied tokens wake the pet"
        );
        // ...but the wake is gentle: backfill cannot fire burst animations.
        assert!(!update.applied_signal.can_burst());
        // And it is bounded: SLEEP_IDLE_MINUTES later with no new rows, back asleep.
        let later = now + Duration::minutes(crate::tui::day::SLEEP_IDLE_MINUTES + 11);
        let resettled = crate::tui::day::build_day_context(&usage, &state, later, mapper);
        assert!(
            resettled.asleep,
            "one wake, then re-sleep after the idle window"
        );
    }

    fn discontinuity_diagnostic_for_test(
        surface: &str,
        recorded_at: OffsetDateTime,
    ) -> crate::storage::usage_store::ProviderDiagnostic {
        crate::storage::usage_store::ProviderDiagnostic {
            provider_surface: surface.to_string(),
            code: crate::game::runtime::USAGE_DISCONTINUITY_CODE.to_string(),
            message: "refused 212000000 effective tokens (threshold 99000000)".to_string(),
            recorded_at,
        }
    }

    #[test]
    fn usage_discontinuity_does_not_mark_a_source_broken() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let diagnostics = vec![discontinuity_diagnostic_for_test("claude-code", now)];

        let today = vec![crate::usage::snapshot::SourceSnapshotHealth {
            accounting_source: "claude-code".to_string(),
            display_name: "claude".to_string(),
            snapshot_state: crate::usage::snapshot::SnapshotState::Current,
            snapshot_total_tokens: Some(12_000.0),
            recent_accepted_tokens: 0.0,
            reason: None,
        }];
        let health = source_health(&today, &diagnostics);
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].status, SourceStatus::Ready);
        assert_eq!(health[0].diagnostic_code, None);
        assert_eq!(health[0].diagnostic_message, None);

        let health = source_health(&[], &diagnostics);
        assert!(
            health.is_empty(),
            "a discontinuity-only surface must not appear broken: {health:?}"
        );
    }

    #[test]
    fn usage_discontinuity_survives_the_ready_today_filter() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let helper_exit = crate::storage::usage_store::ProviderDiagnostic {
            provider_surface: "codex".to_string(),
            code: "helper_exit".to_string(),
            message: "helper exited 2".to_string(),
            recorded_at: now,
        };
        let sources = vec![
            SourceUsageView {
                name: "claude-code".to_string(),
                display_name: "claude".to_string(),
                effective_tokens: 12_000.0,
            },
            SourceUsageView {
                name: "codex".to_string(),
                display_name: "codex".to_string(),
                effective_tokens: 500.0,
            },
        ];

        let active = active_diagnostics(
            &sources,
            vec![
                discontinuity_diagnostic_for_test("claude-code", now),
                helper_exit,
            ],
        );

        assert_eq!(active.len(), 1);
        assert_eq!(
            active[0].code,
            crate::game::runtime::USAGE_DISCONTINUITY_CODE
        );
    }

    #[test]
    fn feed_timestamps_render_the_mapper_local_clock_not_utc() {
        use crate::storage::state::NarrativeEvent;
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        UsageStore::open(&db_path).unwrap();
        let now = PrimitiveDateTime::new(
            Date::from_calendar_date(2026, Month::June, 10).unwrap(),
            Time::from_hms(6, 30, 0).unwrap(),
        )
        .assume_utc();
        let mut state = PetState::new_for_test("test", "buddy");
        state.recent_events = vec![NarrativeEvent {
            observed_at: now - Duration::minutes(30), // 06:00 UTC = 23:00 at UTC-7
            text: "buddy munched 1.0k tokens".into(),
        }];
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::from_hms(-7, 0, 0).unwrap());

        let vm = build_watch_view_model_at(&state, &db_path, now, mapper).unwrap();

        let feed = vm
            .recent_events
            .iter()
            .find(|event| event.text.contains("munched"))
            .unwrap();
        assert_eq!(
            feed.timestamp, "23:00",
            "last night's 23:00 local feed must not display as 06:00 UTC"
        );
    }

    #[test]
    fn vm_build_speech_uses_the_scene_precedence_stack_not_raw_token_munch() {
        use time::macros::datetime;
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut store = UsageStore::open(&db_path).unwrap();
        let now = datetime!(2026-05-11 12:00 UTC); // unix_ts % 30 == 0: visible slot
        store
            .insert_event(&sample_event_at_for_test(
                now - Duration::minutes(5),
                1_800_000.0,
            ))
            .unwrap();
        drop(store);
        let mut state = PetState::new_for_test("test", "buddy");
        state.created_at = now - Duration::days(3);

        let vm = build_watch_view_model_at(
            &state,
            &db_path,
            now,
            LocalDayMapper::Fixed(time::UtcOffset::UTC),
        )
        .unwrap();
        let line = vm.current_speech.expect("visible slot must produce a line");
        let munch = ["yum!", "more!", "tasty!", "delicious", "*chomp*"];
        assert!(
            !munch.contains(&line.as_str()),
            "vm build must not munch on raw recent tokens, got {line}"
        );
    }

    #[test]
    fn rerender_threads_day_context_tiredness_into_blink_cadence() {
        let mut rested = WatchViewModel::fixture();
        rested.pet_render.mood = Mood::Content;
        rested.pet_render.generated_species = Species::Blob;
        rested.pet_render.stage = Stage::S3;
        let mut tired = rested.clone();
        tired.day_context.tiredness = 1.0;

        let closed = crate::pet::render::closed_blink_eyes(Species::Blob);
        let mut rested_blinks = 0;
        let mut tired_blinks = 0;
        for tick in 0..1500_u64 {
            rerender_pet_for_view_model(&mut rested, tick, false, time::OffsetDateTime::UNIX_EPOCH)
                .unwrap();
            if rested.pet_art.join("\n").contains(closed) {
                rested_blinks += 1;
            }
            rerender_pet_for_view_model(&mut tired, tick, false, time::OffsetDateTime::UNIX_EPOCH)
                .unwrap();
            if tired.pet_art.join("\n").contains(closed) {
                tired_blinks += 1;
            }
        }
        assert!(
            tired_blinks > 0,
            "a tired pet still blinks, just less often"
        );
        assert!(
            rested_blinks > tired_blinks,
            "tiredness must slow blinking through the rerender path \
             (app frame tick + menubar animate): {rested_blinks} vs {tired_blinks}"
        );
    }

    #[test]
    fn vm_breath_rhythm_lets_asleep_outrank_tiredness() {
        use crate::pet::animator::{compute_breath_offset_with_rhythm, BreathRhythm};
        use time::macros::datetime;
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut store = UsageStore::open(&db_path).unwrap();
        for back in 2..=6_i64 {
            for hour in [9_i64, 13, 17] {
                let at =
                    datetime!(2026-06-10 00:00 UTC) - Duration::days(back) + Duration::hours(hour);
                store
                    .insert_event(&sample_event_at_for_test(at, 10_000.0))
                    .unwrap();
            }
        }
        for i in 0..24_i64 {
            store
                .insert_event(&sample_event_at_for_test(
                    datetime!(2026-06-09 18:00 UTC) + Duration::minutes(i * 10),
                    20_000.0,
                ))
                .unwrap();
        }
        drop(store);
        let mut state = PetState::new_for_test("test", "buddy");
        state.created_at = datetime!(2026-06-01 00:00 UTC);
        state.pet.generated_species = Species::Crystal;

        let now = datetime!(2026-06-10 01:30 UTC);
        let onset = datetime!(2026-06-10 00:00 UTC);
        let asleep_rhythm = BreathRhythm::Asleep { onset };
        let tired_rhythm = BreathRhythm::Tired { eighths: 2 };
        let probe = (0..180_i64)
            .map(|s| now + Duration::seconds(s))
            .find(|&t| {
                compute_breath_offset_with_rhythm(Some(Species::Crystal), t, asleep_rhythm)
                    != compute_breath_offset_with_rhythm(Some(Species::Crystal), t, tired_rhythm)
            })
            .expect("sleep (18s period) and tired (6.7s) rhythms diverge fast");

        let vm = build_watch_view_model_at(
            &state,
            &db_path,
            probe,
            LocalDayMapper::Fixed(time::UtcOffset::UTC),
        )
        .unwrap();
        assert!(vm.day_context.asleep, "fixture must derive an asleep scene");
        assert!(
            vm.day_context.tiredness > 0.05,
            "fixture must also be tired, got {}",
            vm.day_context.tiredness
        );
        assert_eq!(
            vm.breath_offset_y,
            compute_breath_offset_with_rhythm(Some(Species::Crystal), probe, asleep_rhythm),
            "asleep must outrank tired at the vm breath call site"
        );
    }

    #[test]
    fn habitat_view_carries_unlock_provenance_for_resonance() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        drop(UsageStore::open(&db_path).unwrap());
        let mut state = PetState::new_for_test("seed", "buddy");
        state.habitat.earned_props = vec![crate::storage::state::EarnedHabitatProp {
            id: crate::storage::state::HabitatPropId::new(
                crate::game::habitat::HEAVY_SESSION_PLANTER,
            ),
            earned_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            source: crate::storage::state::HabitatPropSource::HeavySession,
        }];
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_600).unwrap();
        let vm = build_watch_view_model_at(
            &state,
            &db_path,
            now,
            LocalDayMapper::Fixed(time::UtcOffset::UTC),
        )
        .unwrap();
        assert_eq!(
            vm.habitat.earned_props[0].source,
            crate::storage::state::HabitatPropSource::HeavySession
        );
    }

    #[test]
    fn view_model_carries_activity_identity() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut store = UsageStore::open(&db_path).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();

        // Two sources today -> dual-lane.
        let mut claude = NormalizedUsageEvent::for_test_at(now - Duration::minutes(5), 50_000.0);
        claude.provider_surface = "claude-code".into();
        let mut codex = NormalizedUsageEvent::for_test_at(now - Duration::minutes(5), 50_000.0);
        codex.provider_surface = "codex".into();
        store.insert_event(&claude).unwrap();
        store.insert_event(&codex).unwrap();
        let provider_day = crate::usage::day_axis::tokenmaxxing_provider_day(now);
        seed_snapshot_for_test(&mut store, provider_day, "claude-code", 50_000.0, now);
        seed_snapshot_for_test(&mut store, provider_day, "codex", 50_000.0, now);

        drop(store);
        let mut state = PetState::new_for_test("test", "Mochi");
        state.calibration.daily_effective_tokens = 100_000.0;

        let vm = build_watch_view_model_at(
            &state,
            &db_path,
            now,
            LocalDayMapper::Fixed(time::UtcOffset::UTC),
        )
        .unwrap();
        assert_eq!(
            vm.activity_identity.source_diversity,
            SourceDiversity::DualLane
        );
        assert_eq!(
            vm.activity_identity.relative_intensity,
            crate::tui::identity::RelativeIntensity::Normal
        );
    }

    #[test]
    fn rerender_applies_mood_eye_color() {
        use crate::game::metabolism::Mood;
        use crate::pet::palette::eye_color_for_mood;

        let mut vm = WatchViewModel::fixture();

        vm.pet_render.mood = Mood::Sleepy;
        rerender_pet_for_view_model(&mut vm, 1, false, time::OffsetDateTime::UNIX_EPOCH).unwrap();
        assert_eq!(
            vm.pet_palette.eye,
            eye_color_for_mood(Mood::Sleepy),
            "sleepy mood should cool the eye color"
        );

        vm.pet_render.mood = Mood::Ecstatic;
        rerender_pet_for_view_model(&mut vm, 2, false, time::OffsetDateTime::UNIX_EPOCH).unwrap();
        assert_eq!(
            vm.pet_palette.eye,
            eye_color_for_mood(Mood::Ecstatic),
            "ecstatic mood should warm the eye color"
        );
    }
}

#[cfg(test)]
mod glitch_corruption_tests {
    use super::*;
    use crate::game::evolution::Stage;
    use crate::pet::generation::Species;
    use crate::pet::render::PaletteRoleName;
    use crate::storage::{
        state::PetState,
        usage_store::{NormalizedUsageEvent, UsageStore},
    };
    use tempfile::tempdir;
    use time::OffsetDateTime;

    #[test]
    fn glitch_watch_view_model_rerender_adds_day_local_repair_spans() {
        let dir = tempdir().unwrap();
        let usage_db = dir.path().join("usage.sqlite");
        let mut state = PetState::new_for_test("glitch-watch-patches", "Mux");
        state.pet.generated_species = Species::Glitch;
        state.stage = Stage::S6;
        state.calibration.daily_effective_tokens = 10_000.0;
        let now = OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap();
        let mut store = UsageStore::open(&usage_db).unwrap();
        let mut event = NormalizedUsageEvent::for_test_at(now, 18_000.0);
        event.provider_surface = "codex".to_string();
        store.insert_event(&event).unwrap();

        let vm = build_watch_view_model_for_test_at(&state, &usage_db, now).unwrap();

        assert!(
            vm.pet_spans.iter().any(|span| {
                matches!(
                    span.role,
                    PaletteRoleName::Pattern | PaletteRoleName::Accent
                ) && span.end == span.start + 1
            }),
            "Glitch watch VM should include soft one-cell repair spans"
        );
        assert!(
            vm.pet_spans
                .iter()
                .all(|span| span.role != PaletteRoleName::Corruption),
            "day-local repair marks should not use Corruption in the steady VM"
        );
    }

    #[test]
    fn non_glitch_watch_view_model_does_not_receive_glitch_repair_spans() {
        let dir = tempdir().unwrap();
        let usage_db = dir.path().join("usage.sqlite");
        let mut state = PetState::new_for_test("fuzz-watch-patches", "Mochi");
        state.pet.generated_species = Species::Fuzz;
        state.stage = Stage::S6;
        let now = OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap();
        let _store = UsageStore::open(&usage_db).unwrap();

        let vm = build_watch_view_model_for_test_at(&state, &usage_db, now).unwrap();

        assert_eq!(vm.pet_render.generated_species, Species::Fuzz);
        assert!(
            vm.pet_spans
                .iter()
                .all(|span| span.role != PaletteRoleName::Corruption),
            "non-Glitch steady VM should not get Glitch corruption roles"
        );
    }
}
