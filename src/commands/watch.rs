use crate::{
    error::{GlorpError, Result},
    game::{
        evolution::Stage,
        metabolism::{apply_food, Mood, Vitals as GameVitals},
        runtime::apply_usage_poll,
    },
    paths::AppPaths,
    pet::{
        art::stage_label,
        generation::{generate_pet, Species},
        render::{render_pet, AnimationFrame},
    },
    storage::{
        state::{PetState, StateStore},
        usage_store::{NormalizedUsageEvent, UsageStore},
    },
    tui::{
        app::{WatchApp, WatchUsagePoller},
        style::LogKind,
        view_model::{EventView, SourceUsageView, WatchViewModel},
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

    let state = poll_usage_and_apply(&state_store, &paths.usage_db)?.unwrap_or(state);
    let vm = build_watch_view_model(&state, &paths.usage_db)?;
    WatchApp::with_poll_callback(
        vm,
        Default::default(),
        Box::new(RealWatchPoller {
            state_file: paths.state_file,
            usage_db: paths.usage_db,
        }),
    )
    .run()
}

pub fn build_watch_view_model(state: &PetState, usage_db: &Path) -> Result<WatchViewModel> {
    let usage_store = UsageStore::open(usage_db)?;
    let recent_usage = usage_store.recent_events(500)?;
    let now = OffsetDateTime::now_utc();
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
            compact: false,
            blink_suppression_ticks: 0,
        },
    );

    let source_breakdown = source_breakdown(&recent_usage, now);
    let diagnostics = active_diagnostics(&source_breakdown, usage_store.recent_diagnostics(5)?);
    let helper_status = helper_status(&usage_store, &source_breakdown, &diagnostics)?;
    let recent_events = build_recent_events(state, &recent_usage, &diagnostics);
    let errors = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect::<Vec<_>>();

    Ok(WatchViewModel {
        pet_art: rendered.lines,
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
        recent_daily_effective_tokens: recent_daily_effective_tokens(&recent_usage, now),
        source_breakdown,
        current_bucket_effective_tokens: current_bucket_effective_tokens(&recent_usage, now),
        recent_events,
        helper_status,
        errors,
        latest_evolution: state.seen_stage_transitions.last().cloned(),
    })
}

#[doc(hidden)]
pub fn build_watch_view_model_for_test(
    state: &PetState,
    usage_db: &Path,
) -> Result<WatchViewModel> {
    build_watch_view_model(state, usage_db)
}

struct RealWatchPoller {
    state_file: std::path::PathBuf,
    usage_db: std::path::PathBuf,
}

impl WatchUsagePoller for RealWatchPoller {
    fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchViewModel> {
        let state_store = StateStore::new(self.state_file.clone());
        let state = match poll_usage_and_apply(&state_store, &self.usage_db) {
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

fn poll_usage_and_apply(state_store: &StateStore, usage_db: &Path) -> Result<Option<PetState>> {
    let Some(mut state) = state_store.load()? else {
        return Ok(None);
    };
    let mut usage_store = UsageStore::open(usage_db)?;
    let result = CcusageCommandProvider::from_environment().poll(&mut usage_store)?;
    if !result.deltas.is_empty() || result.diagnostics.is_empty() {
        apply_usage_poll(
            &mut state,
            &mut usage_store,
            &result,
            OffsetDateTime::now_utc(),
        )?;
        state_store.save(&state)?;
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

fn next_stage_xp_target(stage: Stage) -> f64 {
    match stage {
        Stage::S0 => 0.25,
        Stage::S1 => 1.0,
        Stage::S2 => 3.0,
        Stage::S3 => 7.0,
        Stage::S4 => 21.0,
        Stage::S5 | Stage::S6 => 49.0,
    }
}

fn today_effective_tokens(events: &[NormalizedUsageEvent], now: OffsetDateTime) -> f64 {
    let today = now.date();
    events
        .iter()
        .filter(|event| event.period_start.date() == today)
        .map(|event| event.effective_tokens)
        .sum()
}

fn current_bucket_effective_tokens(events: &[NormalizedUsageEvent], now: OffsetDateTime) -> f64 {
    let cutoff = now - Duration::minutes(10);
    events
        .iter()
        .filter(|event| event.period_start >= cutoff)
        .map(|event| event.effective_tokens)
        .sum()
}

fn recent_daily_effective_tokens(events: &[NormalizedUsageEvent], now: OffsetDateTime) -> Vec<f64> {
    let mut by_day = BTreeMap::new();
    for event in events {
        *by_day.entry(event.period_start.date()).or_insert(0.0) += event.effective_tokens;
    }

    (0..7)
        .rev()
        .map(|days_ago| {
            let day = now.date() - Duration::days(days_ago);
            by_day.get(&day).copied().unwrap_or(0.0)
        })
        .collect()
}

fn source_breakdown(events: &[NormalizedUsageEvent], now: OffsetDateTime) -> Vec<SourceUsageView> {
    let today = now.date();
    let mut by_source = BTreeMap::new();
    for event in events
        .iter()
        .filter(|event| event.period_start.date() == today)
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
) -> Vec<EventView> {
    let mut events = Vec::new();
    for event in state.recent_events.iter().rev().take(3).rev() {
        events.push(EventView {
            timestamp: "--:--".into(),
            kind: LogKind::Normal,
            text: event.clone(),
        });
    }
    for event in usage_events.iter().rev().take(4).rev() {
        events.push(EventView {
            timestamp: timestamp_column(event.period_start),
            kind: LogKind::Usage,
            text: format!(
                "{} added {} effective tokens",
                event.provider_surface,
                format_tokens(event.effective_tokens)
            ),
        });
    }
    for diagnostic in diagnostics.iter().rev().take(2).rev() {
        events.push(EventView {
            timestamp: timestamp_column(diagnostic.recorded_at),
            kind: LogKind::Diagnostic,
            text: format!("{}: {}", diagnostic.provider_surface, diagnostic.code),
        });
    }
    events
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
    if value.abs() >= 1_000.0 {
        format!("{:.1}k", value / 1_000.0)
    } else {
        format!("{value:.0}")
    }
}
