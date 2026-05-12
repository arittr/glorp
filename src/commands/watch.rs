use crate::{
    error::{GlorpError, Result},
    game::{
        evolution::Stage,
        metabolism::{apply_food, Mood, Vitals as GameVitals},
        runtime::{apply_unapplied_usage, stage_usage_poll_deltas},
    },
    paths::AppPaths,
    pet::{
        generation::{generate_pet, stage_label, Species},
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
use std::{collections::BTreeMap, path::Path};
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
    let species = parse_species(&state.pet.generated_species)
        .unwrap_or_else(|| generate_pet(&state.pet.seed).species);
    let stage = parse_stage(&state.stage);
    let mood = mood_from_state(state);
    let generated = generate_pet(&state.pet.seed).with_species_for_test(species);
    let rendered = render_pet(
        &generated,
        stage,
        mood,
        AnimationFrame {
            tick: now.unix_timestamp().max(0) as u64,
            blink_suppression_ticks: 0,
        },
    );

    let source_breakdown = source_breakdown(&recent_usage, now);
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
    let source_health = source_health(&recent_usage, &all_diagnostics, now);
    let diagnostics = active_diagnostics(&source_breakdown, all_diagnostics);
    let helper_status = helper_status(&usage_store, &source_breakdown, &diagnostics)?;
    let pet_activities = crate::pet::activity::derive_pet_activities(
        &state.pet.accepted_name,
        species,
        mood_label(mood),
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
            generated_species: state.pet.generated_species.clone(),
            stage: state.stage.clone(),
            mood: mood_label(mood).to_string(),
        },
        pet_name: state.pet.accepted_name.clone(),
        species: species.as_str().to_string(),
        stage: stage_label(species, stage).to_string(),
        mood: mood_label(mood).to_string(),
        age_days: (now - state.created_at).whole_days().max(0) as u32,
        xp_current: state.xp,
        xp_target: next_stage_xp_target(stage),
        fed: state.vitals.fed / 100.0,
        happiness: state.vitals.happiness / 100.0,
        energy: state.vitals.energy / 100.0,
        today_effective_tokens: today_effective_tokens(&recent_usage, now),
        recent_daily_effective_tokens: usage_store
            .seven_day_token_history(now.date())
            .unwrap_or_else(|_| vec![0.0; 7]),
        source_breakdown,
        source_health,
        current_bucket_effective_tokens: current_bucket_effective_tokens(&recent_usage, now),
        recent_events,
        helper_status,
        errors,
        latest_evolution: state.seen_stage_transitions.last().cloned(),
        acknowledged_evolution: None,
        cursor_screen: None,
        mouse_tracking_enabled: true,
        current_speech: crate::pet::speech::current_pet_speech(
            mood_label(mood),
            recent_tokens_per_min(&recent_usage, now),
            now,
        ),
        wander_offset_x: crate::pet::animator::compute_wander_offset(now),
        breath_offset_y: crate::pet::animator::compute_breath_offset(Some(species), now),
        // TODO(Task 8): replace with real ProgressView computed from state
        progress: ProgressView {
            stage_label: stage_label(species, stage).to_string(),
            next_stage_label: String::new(),
            fraction: 0.0,
            xp_in_stage: 0.0,
            xp_to_next: 1.0,
            rate_per_hour: 0.0,
            is_max_stage: false,
        },
        // TODO(Task 9): replace with real BioView computed from state
        bio: BioView {
            hatched_label: String::new(),
            age_label: String::new(),
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

fn parse_species(value: &str) -> Option<Species> {
    match value {
        "fuzz" => Some(Species::Fuzz),
        "blob" => Some(Species::Blob),
        "ghost" => Some(Species::Ghost),
        "glitch" => Some(Species::Glitch),
        "crystal" => Some(Species::Crystal),
        "mech" => Some(Species::Mech),
        _ => None,
    }
}

fn parse_stage(value: &str) -> Stage {
    match value {
        "s1" => Stage::S1,
        "s2" => Stage::S2,
        "s3" => Stage::S3,
        "s4" => Stage::S4,
        "s5" => Stage::S5,
        "s6" => Stage::S6,
        _ => Stage::S0,
    }
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

fn mood_label(mood: Mood) -> &'static str {
    match mood {
        Mood::Happy => "happy",
        Mood::Content => "content",
        Mood::Hungry => "hungry",
        Mood::Sad => "sad",
        Mood::Sleepy => "sleepy",
        Mood::Wilted => "wilted",
    }
}

fn parse_mood(value: &str) -> Mood {
    match value {
        "happy" => Mood::Happy,
        "hungry" => Mood::Hungry,
        "sad" => Mood::Sad,
        "sleepy" => Mood::Sleepy,
        "wilted" => Mood::Wilted,
        _ => Mood::Content,
    }
}

pub fn rerender_pet_for_view_model(vm: &mut WatchViewModel, tick: u64) -> Result<()> {
    let species = parse_species(&vm.pet_render.generated_species)
        .unwrap_or_else(|| generate_pet(&vm.pet_render.seed).species);
    let stage = parse_stage(&vm.pet_render.stage);
    let mood = parse_mood(&vm.pet_render.mood);
    let generated = generate_pet(&vm.pet_render.seed).with_species_for_test(species);
    let rendered = render_pet(
        &generated,
        stage,
        mood,
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

fn today_effective_tokens(events: &[NormalizedUsageEvent], now: OffsetDateTime) -> f64 {
    let today = now.date();
    events
        .iter()
        .filter(|event| event.bucket_at.date() == today)
        .map(|event| event.effective_tokens)
        .sum()
}

fn current_bucket_effective_tokens(events: &[NormalizedUsageEvent], now: OffsetDateTime) -> f64 {
    let cutoff = now - Duration::minutes(10);
    events
        .iter()
        .filter(|event| event.bucket_at >= cutoff)
        .map(|event| event.effective_tokens)
        .sum()
}

fn source_breakdown(events: &[NormalizedUsageEvent], now: OffsetDateTime) -> Vec<SourceUsageView> {
    let today = now.date();
    let mut by_source = BTreeMap::new();
    for event in events
        .iter()
        .filter(|event| event.bucket_at.date() == today)
    {
        *by_source
            .entry(event.provider_surface.clone())
            .or_insert(0.0) += event.effective_tokens;
    }
    by_source
        .into_iter()
        .map(|(name, effective_tokens)| SourceUsageView {
            name,
            effective_tokens,
        })
        .collect()
}

fn source_health(
    events: &[NormalizedUsageEvent],
    diagnostics: &[crate::storage::usage_store::ProviderDiagnostic],
    now: OffsetDateTime,
) -> Vec<SourceHealthView> {
    let mut names = std::collections::BTreeSet::new();
    for event in events {
        names.insert(event.provider_surface.clone());
    }
    for diagnostic in diagnostics {
        names.insert(diagnostic.provider_surface.clone());
    }

    let bucket_cutoff = now - Duration::minutes(10);
    let today = now.date();
    names
        .into_iter()
        .map(|name| {
            let today_effective_tokens = events
                .iter()
                .filter(|event| event.provider_surface == name && event.bucket_at.date() == today)
                .map(|event| event.effective_tokens)
                .sum::<f64>();
            let bucket_effective_tokens = events
                .iter()
                .filter(|event| event.provider_surface == name && event.bucket_at >= bucket_cutoff)
                .map(|event| event.effective_tokens)
                .sum::<f64>();
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

fn format_tokens(value: f64) -> String {
    let value = value.max(0.0);
    if value >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("{:.1}k", value / 1_000.0)
    } else {
        format!("{value:.0}")
    }
}
