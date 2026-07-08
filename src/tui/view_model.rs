use crate::game::metabolism::Mood;
use crate::game::{
    evolution::Stage,
    habitat::{HabitatPropKind, TankInhabitantKind},
};
use crate::pet::generation::Species;
use crate::storage::state::{
    HabitatPropId, HabitatPropSource, TankInhabitantId, TankInhabitantSource,
};
use crate::tui::identity::ActivityIdentityProfile;
use crate::tui::life::PetLifeProfile;
use crate::tui::style::LogKind;

#[derive(Debug, Clone, PartialEq)]
pub struct WatchViewModel {
    pub pet_art: Vec<String>,
    pub pet_spans: Vec<crate::pet::render::StyledSegment>,
    pub pet_render: PetRenderModel,
    pub pet_palette: crate::pet::palette::ResolvedPalette,
    pub habitat: HabitatView,
    pub life_profile: PetLifeProfile,
    pub activity_identity: ActivityIdentityProfile,
    /// Derived time-of-day context. Built once per poll alongside the vm;
    /// per-frame consumers compare clock instants against its UTC boundaries.
    pub day_context: crate::tui::day::DayContext,
    pub pet_name: String,
    pub species: String,
    pub stage: String,
    pub mood: String,
    pub age_days: u32,
    pub fed: f64,
    pub happiness: f64,
    pub energy: f64,
    /// Transitional field name. Values are canonical Tokenmaxxing total tokens
    /// for tokenmaxxing_total_v1 rows.
    pub today_effective_tokens: f64,
    pub today_snapshot_state: crate::usage::snapshot::SnapshotState,
    pub today_snapshot_reason: Option<String>,
    pub daily_comparison: DailyComparison,
    pub recent_daily_effective_tokens: Vec<f64>,
    pub recent_daily_snapshot_states: Vec<crate::usage::snapshot::SnapshotState>,
    pub source_breakdown: Vec<SourceUsageView>,
    pub source_health: Vec<SourceHealthView>,
    pub current_bucket_effective_tokens: f64,
    pub rate_momentum: RateMomentum,
    pub recent_events: Vec<EventView>,
    pub helper_status: String,
    pub errors: Vec<String>,
    pub latest_evolution: Option<String>,
    /// Raw mouse position in screen coordinates, or None when the cursor is
    /// off the terminal or mouse tracking is disabled. PetPanel hit-tests
    /// against its rect to decide whether to swap to cursor-tracked eyes.
    pub cursor_screen: Option<(u16, u16)>,
    /// Whether mouse tracking is currently enabled (toggled via `m` key).
    pub mouse_tracking_enabled: bool,
    /// Pet's current speech-bubble line, or None when the bubble is hidden.
    /// Computed deterministically from mood + recent token activity + wall
    /// clock; cycles ~5s visible / ~25s hidden.
    pub current_speech: Option<String>,
    /// Column offset for the idle-wander animation. In the renderer the panel
    /// calls `compute_wander_position_x` using the live `area.width`, so this
    /// field is not used for real rendering. It is set in the view model for
    /// integration tests that need to inspect or override the position.
    /// Updated each frame from wall clock by the watch app; fixtures default
    /// to 0 to keep snapshot tests stable.
    pub wander_offset_x: i16,
    /// Row offset for the idle-breathing animation. 0 = resting (lower
    /// position), 1 = peak inhale (raised one row). PetPanel reserves a
    /// fixed extra row above the art so the shift fits without clipping.
    pub breath_offset_y: u8,
    /// Facing direction: +1 = facing right (default), -1 = facing left.
    /// Derived from the sign of the drift target's velocity. The renderer
    /// horizontally mirrors the pet art when this is -1.
    pub facing: i8,
    /// Wall-clock time of the most recent live-profile burst stamped by the
    /// poll owner. Drives the 2-second token-gain pop overlay in color modes.
    /// None until the first qualifying live poll, and always None in Flat mode.
    pub last_feed_pulse_at: Option<time::OffsetDateTime>,
    /// Stage progress data for the progress panel.
    pub progress: ProgressView,
    /// Birth / age metadata for the bio card panel.
    pub bio: BioView,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DailyComparison {
    pub today_provider_day: time::Date,
    pub yesterday_provider_day: time::Date,
    pub today_tokens: f64,
    pub yesterday_tokens: Option<f64>,
    pub today_snapshot_state: crate::usage::snapshot::SnapshotState,
    pub yesterday_snapshot_state: crate::usage::snapshot::SnapshotState,
    pub today_observed_at: Option<time::OffsetDateTime>,
    pub yesterday_observed_at: Option<time::OffsetDateTime>,
    pub unavailable_reason: Option<String>,
    pub fraction_of_yesterday: Option<f64>,
}

impl DailyComparison {
    pub fn from_snapshots(
        today: &crate::usage::snapshot::SnapshotResult<crate::usage::snapshot::DayTotals>,
        yesterday: &crate::usage::snapshot::SnapshotResult<crate::usage::snapshot::DayTotals>,
    ) -> Self {
        let today_tokens = snapshot_total_tokens(today).unwrap_or(0.0);
        let yesterday_tokens = snapshot_total_tokens(yesterday);
        let unavailable_reason = daily_unavailable_reason(today, yesterday);
        let fraction_of_yesterday = unavailable_reason
            .as_ref()
            .is_none()
            .then(|| today_tokens / yesterday_tokens.expect("validated yesterday total"));

        Self {
            today_provider_day: today.provider_day,
            yesterday_provider_day: yesterday.provider_day,
            today_tokens,
            yesterday_tokens,
            today_snapshot_state: today.state,
            yesterday_snapshot_state: yesterday.state,
            today_observed_at: today.observed_at,
            yesterday_observed_at: yesterday.observed_at,
            unavailable_reason,
            fraction_of_yesterday,
        }
    }
}

fn snapshot_total_tokens(
    result: &crate::usage::snapshot::SnapshotResult<crate::usage::snapshot::DayTotals>,
) -> Option<f64> {
    result.value.as_ref().map(|totals| totals.total_tokens)
}

fn daily_unavailable_reason(
    today: &crate::usage::snapshot::SnapshotResult<crate::usage::snapshot::DayTotals>,
    yesterday: &crate::usage::snapshot::SnapshotResult<crate::usage::snapshot::DayTotals>,
) -> Option<String> {
    use crate::usage::snapshot::SnapshotState;

    if today.state != SnapshotState::Current {
        return Some(format!("today-{}", snapshot_state_slug(today.state)));
    }
    if yesterday.state != SnapshotState::Current {
        return Some(format!(
            "yesterday-{}",
            snapshot_state_slug(yesterday.state)
        ));
    }

    let Some(today_tokens) = snapshot_total_tokens(today) else {
        return Some("today-missing".to_string());
    };
    let Some(yesterday_tokens) = snapshot_total_tokens(yesterday) else {
        return Some("yesterday-missing".to_string());
    };

    if !today_tokens.is_finite()
        || !yesterday_tokens.is_finite()
        || today_tokens < 0.0
        || yesterday_tokens < 0.0
    {
        return Some("non-finite-total".to_string());
    }
    if yesterday_tokens == 0.0 {
        return Some("yesterday-zero".to_string());
    }

    None
}

fn snapshot_state_slug(state: crate::usage::snapshot::SnapshotState) -> &'static str {
    match state {
        crate::usage::snapshot::SnapshotState::Current => "current",
        crate::usage::snapshot::SnapshotState::Stale => "stale",
        crate::usage::snapshot::SnapshotState::Missing => "missing",
        crate::usage::snapshot::SnapshotState::Blocked => "blocked",
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PetRenderModel {
    pub seed: String,
    pub generated_species: Species,
    pub stage: Stage,
    pub mood: Mood,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HabitatView {
    pub earned_props: Vec<EarnedHabitatPropView>,
    pub earned_inhabitants: Vec<EarnedTankInhabitantView>,
    pub tank_life_local_date: time::Date,
    pub tank_life_calendar_age_days: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EarnedHabitatPropView {
    pub id: HabitatPropId,
    pub earned_at: time::OffsetDateTime,
    pub kind: HabitatPropKind,
    pub display_priority: i16,
    pub source: HabitatPropSource,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EarnedTankInhabitantView {
    pub id: TankInhabitantId,
    pub earned_at: time::OffsetDateTime,
    pub unlock_age_days: i64,
    pub kind: TankInhabitantKind,
    pub source: TankInhabitantSource,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceUsageView {
    pub name: String,
    pub display_name: String,
    /// Transitional field name. Values are canonical Tokenmaxxing total tokens
    /// for tokenmaxxing_total_v1 rows.
    pub effective_tokens: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStatus {
    Ready,
    Diagnostic,
    Blocked,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceHealthView {
    pub name: String,
    pub display_name: String,
    pub status: SourceStatus,
    pub snapshot_state: crate::usage::snapshot::SnapshotState,
    pub snapshot_reason: Option<String>,
    /// Transitional field name. Values are canonical Tokenmaxxing total tokens
    /// for tokenmaxxing_total_v1 rows.
    pub today_effective_tokens: f64,
    /// Transitional field name. Values are canonical Tokenmaxxing total tokens
    /// for tokenmaxxing_total_v1 rows.
    pub bucket_effective_tokens: f64,
    pub diagnostic_code: Option<String>,
    pub diagnostic_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventView {
    pub timestamp: String,
    pub kind: LogKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateDirection {
    Up,
    Down,
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateWindow {
    pub current_tokens: f64,
    pub previous_tokens: f64,
    pub direction: RateDirection,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateMomentum {
    pub pulse: RateWindow,
    pub hour: RateWindow,
    pub companion_direction: RateDirection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressView {
    /// Current stage's species-specific label (e.g. "shard", "fractal").
    pub stage_label: String,
    /// Next stage's label, or "—" at S6.
    pub next_stage_label: String,
    /// 0.0..=1.0; saturates at 1.0.
    pub fraction: f32,
    /// state.xp - stage_start_xp(state.stage), in stage-progress units.
    pub xp_in_stage: f64,
    /// next_stage_xp_target(state.stage) - stage_start_xp(state.stage), in
    /// stage-progress units (the current stage's full span).
    pub xp_to_next: f64,
    /// Effective tokens observed in the last hour (sum over `bucket_at`).
    pub rate_per_hour: f64,
    /// True at S6; ProgressPanel renders "max evolved" instead of a bar.
    pub is_max_stage: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BioView {
    /// Pre-formatted "may 11 04:00" — local TZ, computed at vm-build time.
    pub hatched_label: String,
    /// "0d 4h" if < 24h, otherwise "12d".
    pub age_label: String,
}

impl BioView {
    pub fn format_age(age: time::Duration) -> String {
        let total_hours = age.whole_hours();
        if total_hours < 24 {
            format!("0d {total_hours}h")
        } else {
            let days = age.whole_days();
            format!("{days}d")
        }
    }
}

impl WatchViewModel {
    pub fn fixture() -> Self {
        Self {
            pet_art: vec!["  /\\_/\\  ".into(), " ( o.o ) ".into(), "  > ^ <  ".into()],
            pet_spans: Vec::new(),
            pet_render: PetRenderModel {
                seed: "fixture-seed".into(),
                generated_species: Species::Fuzz,
                stage: Stage::S0,
                mood: Mood::Content,
            },
            pet_palette: crate::pet::palette::default_theme_palette(),
            habitat: HabitatView {
                earned_props: Vec::new(),
                earned_inhabitants: Vec::new(),
                tank_life_local_date: time::Date::from_calendar_date(1970, time::Month::January, 1)
                    .unwrap(),
                tank_life_calendar_age_days: 0,
            },
            life_profile: PetLifeProfile::default(),
            activity_identity: ActivityIdentityProfile::default(),
            day_context: crate::tui::day::DayContext::default(),
            pet_name: "miso".into(),
            species: "terminal sprout".into(),
            stage: "hatchling".into(),
            mood: "curious".into(),
            age_days: 4,
            fed: 0.72,
            happiness: 0.64,
            energy: 0.81,
            today_effective_tokens: 18_420.0,
            today_snapshot_state: crate::usage::snapshot::SnapshotState::Current,
            today_snapshot_reason: None,
            daily_comparison: DailyComparison {
                today_provider_day: time::macros::date!(2026 - 07 - 06),
                yesterday_provider_day: time::macros::date!(2026 - 07 - 05),
                today_tokens: 18_420.0,
                yesterday_tokens: Some(16_000.0),
                today_snapshot_state: crate::usage::snapshot::SnapshotState::Current,
                yesterday_snapshot_state: crate::usage::snapshot::SnapshotState::Current,
                today_observed_at: None,
                yesterday_observed_at: None,
                unavailable_reason: None,
                fraction_of_yesterday: Some(18_420.0 / 16_000.0),
            },
            recent_daily_effective_tokens: vec![
                1_000.0, 8_000.0, 4_000.0, 13_000.0, 9_500.0, 16_000.0, 18_420.0,
            ],
            recent_daily_snapshot_states: vec![crate::usage::snapshot::SnapshotState::Current; 7],
            source_breakdown: vec![
                SourceUsageView {
                    name: "claude-code".into(),
                    display_name: "claude".into(),
                    effective_tokens: 12_900.0,
                },
                SourceUsageView {
                    name: "codex".into(),
                    display_name: "codex".into(),
                    effective_tokens: 5_520.0,
                },
            ],
            source_health: vec![
                SourceHealthView {
                    name: "claude-code".into(),
                    display_name: "claude".into(),
                    status: SourceStatus::Ready,
                    snapshot_state: crate::usage::snapshot::SnapshotState::Current,
                    snapshot_reason: None,
                    today_effective_tokens: 12_900.0,
                    bucket_effective_tokens: 1_300.0,
                    diagnostic_code: None,
                    diagnostic_message: None,
                },
                SourceHealthView {
                    name: "codex".into(),
                    display_name: "codex".into(),
                    status: SourceStatus::Ready,
                    snapshot_state: crate::usage::snapshot::SnapshotState::Current,
                    snapshot_reason: None,
                    today_effective_tokens: 5_520.0,
                    bucket_effective_tokens: 1_000.0,
                    diagnostic_code: None,
                    diagnostic_message: None,
                },
            ],
            current_bucket_effective_tokens: 2_300.0,
            rate_momentum: RateMomentum {
                pulse: RateWindow {
                    current_tokens: 2_300.0,
                    previous_tokens: 900.0,
                    direction: RateDirection::Up,
                },
                hour: RateWindow {
                    current_tokens: 109_000.0,
                    previous_tokens: 140_000.0,
                    direction: RateDirection::Down,
                },
                companion_direction: RateDirection::Up,
            },
            recent_events: vec![
                EventView {
                    timestamp: "13:38".into(),
                    kind: LogKind::Normal,
                    text: "watch loop settled; next poll in 60s".into(),
                },
                EventView {
                    timestamp: "13:40".into(),
                    kind: LogKind::Usage,
                    text: "fed from 2.3k effective tokens".into(),
                },
            ],
            helper_status: "helper ready".into(),
            errors: Vec::new(),
            latest_evolution: None,
            cursor_screen: None,
            mouse_tracking_enabled: true,
            current_speech: None,
            wander_offset_x: 0,
            breath_offset_y: 0,
            facing: 1,
            last_feed_pulse_at: None,
            progress: ProgressView {
                stage_label: "fuzz".to_string(),
                next_stage_label: "archfuzz".to_string(),
                fraction: 0.45,
                xp_in_stage: 4.5,
                xp_to_next: 10.0,
                rate_per_hour: 109_000.0,
                is_max_stage: false,
            },
            bio: BioView {
                hatched_label: "apr 24 14:32".to_string(),
                age_label: "18d".to_string(),
            },
        }
    }

    pub fn fixture_with_events() -> Self {
        let mut vm = Self::fixture();
        vm.recent_events = vec![
            EventView {
                timestamp: "13:42".into(),
                kind: LogKind::Usage,
                text: "claude-code added 1.3k effective tokens".into(),
            },
            EventView {
                timestamp: "13:42".into(),
                kind: LogKind::Diagnostic,
                text: "ccusage helper returned a retryable diagnostic".into(),
            },
            EventView {
                timestamp: "13:42".into(),
                kind: LogKind::Evolution,
                text: "miso shimmered toward sproutling".into(),
            },
        ];
        vm.latest_evolution = Some("sproutling shimmer ready".into());
        vm
    }

    #[doc(hidden)]
    pub fn fixture_with_habitat_props() -> Self {
        let mut vm = Self::fixture();
        vm.habitat.earned_props = vec![
            EarnedHabitatPropView {
                id: crate::storage::state::HabitatPropId::new("codex_signal_lamp"),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: crate::game::habitat::HabitatPropKind::Trophy,
                display_priority: 70,
                source: HabitatPropSource::ProviderFirstUse {
                    provider_surface: "codex".to_string(),
                },
            },
            EarnedHabitatPropView {
                id: crate::storage::state::HabitatPropId::new("token_pebble_25k"),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: crate::game::habitat::HabitatPropKind::Accent,
                display_priority: 10,
                source: HabitatPropSource::LifetimeTokens { threshold: 25_000.0 },
            },
        ];
        vm
    }

    #[doc(hidden)]
    pub fn fixture_with_tank_inhabitants_for_age(age_days: i64, local_date: time::Date) -> Self {
        let mut vm = Self::fixture_with_habitat_props();
        vm.habitat.tank_life_local_date = local_date;
        vm.habitat.tank_life_calendar_age_days = age_days;
        vm.habitat.earned_inhabitants = crate::game::habitat::TANK_INHABITANT_CATALOG
            .iter()
            .filter(|spec| spec.unlock_age_days <= age_days)
            .map(|spec| EarnedTankInhabitantView {
                id: TankInhabitantId::new(spec.id),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                unlock_age_days: spec.unlock_age_days,
                kind: spec.kind,
                source: TankInhabitantSource::PetAgeThreshold {
                    threshold_days: spec.unlock_age_days,
                },
            })
            .collect();
        vm
    }

    /// Like `fixture_with_events` but fills `recent_events` with `n` Usage events
    /// (alternating claude-code / codex sources) for cap and color tests.
    #[doc(hidden)]
    pub fn fixture_with_n_events(n: usize) -> Self {
        let mut vm = Self::fixture();
        vm.recent_events = (0..n)
            .map(|i| {
                let source = if i % 2 == 0 { "claude-code" } else { "codex" };
                EventView {
                    timestamp: format!("13:{:02}", i % 60),
                    kind: LogKind::Usage,
                    text: format!("{source} added 1.0k effective tokens"),
                }
            })
            .collect();
        vm
    }

    pub fn is_blocked(&self) -> bool {
        !self.source_health.is_empty()
            && self.source_health.iter().all(|source| {
                matches!(
                    source.status,
                    SourceStatus::Blocked | SourceStatus::Diagnostic
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    #[test]
    fn progress_view_has_required_fields() {
        let p = ProgressView {
            stage_label: "shard".to_string(),
            next_stage_label: "fractal".to_string(),
            fraction: 0.33,
            xp_in_stage: 0.33,
            xp_to_next: 1.0,
            rate_per_hour: 109_000.0,
            is_max_stage: false,
        };
        assert_eq!(p.stage_label, "shard");
        assert_eq!(p.next_stage_label, "fractal");
        assert!((p.fraction - 0.33).abs() < 1e-6);
        assert!(!p.is_max_stage);
    }

    #[test]
    fn progress_view_at_max_stage() {
        let p = ProgressView {
            stage_label: "aurora".to_string(),
            next_stage_label: "—".to_string(),
            fraction: 1.0,
            xp_in_stage: 60.0,
            xp_to_next: 60.0,
            rate_per_hour: 0.0,
            is_max_stage: true,
        };
        assert!(p.is_max_stage);
    }

    #[test]
    fn bio_view_age_label_sub_day_formats_as_hours() {
        assert_eq!(BioView::format_age(Duration::ZERO), "0d 0h");
        assert_eq!(BioView::format_age(Duration::hours(1)), "0d 1h");
        assert_eq!(BioView::format_age(Duration::hours(23)), "0d 23h");
    }

    #[test]
    fn bio_view_age_label_day_or_more_drops_hours() {
        assert_eq!(BioView::format_age(Duration::hours(24)), "1d");
        assert_eq!(BioView::format_age(Duration::hours(25)), "1d");
        assert_eq!(BioView::format_age(Duration::days(7)), "7d");
        assert_eq!(BioView::format_age(Duration::days(90)), "90d");
        assert_eq!(BioView::format_age(Duration::days(365)), "365d");
    }

    #[test]
    fn watch_view_model_fixture_has_progress_and_bio() {
        let vm = WatchViewModel::fixture();
        assert!(
            !vm.progress.stage_label.is_empty(),
            "progress.stage_label must be non-empty in fixture"
        );
        assert!(
            !vm.bio.hatched_label.is_empty(),
            "bio.hatched_label must be non-empty in fixture"
        );
        assert_eq!(
            vm.life_profile,
            crate::tui::life::PetLifeProfile::default(),
            "fixture should start with the quiet live profile"
        );
    }
}
