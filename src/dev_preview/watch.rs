use crate::commands::watch::{build_watch_view_model_at, rerender_pet_for_view_model};
use crate::dev_preview::frame::{frame_from_buffer, PreviewFrame};
use crate::dev_preview::scenarios::PreviewRenderContext;
use crate::error::Result;
use crate::game::evolution::Stage;
use crate::pet::generation::Species;
use crate::storage::{
    state::{
        EarnedHabitatProp, HabitatPropId, HabitatPropSource, NarrativeEvent, PetState, Vitals,
    },
    usage_store::{NormalizedUsageEvent, UsageStore},
};
use crate::tui::day::{DayContext, DayPhase, DaySummary, WakeResume};
use crate::tui::layout::render_watch_frame_with_layout;
use crate::tui::life::{
    IdleLifeState, PetLifeProfile, PropReaction, PropReactionKind, SourceAccent, WorkWeather,
};
use crate::tui::render_context::{RenderContext, WatchClock};
use crate::tui::style::ColorCapability;
use crate::tui::{component::layout_watch_with_context, component::preview_layout};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::path::Path;
use time::{Duration, OffsetDateTime, UtcOffset};

pub fn watch_frames(ctx: &PreviewRenderContext, scratch_dir: &Path) -> Result<Vec<PreviewFrame>> {
    std::fs::create_dir_all(scratch_dir)?;

    let mut frames = vec![
        render_watch_frame(
            "watch-wide-normal",
            "Watch Wide Normal",
            120,
            32,
            ctx,
            scratch_dir,
        )?,
        render_watch_frame(
            "watch-tall-wide",
            "Watch Tall Wide",
            180,
            50,
            ctx,
            scratch_dir,
        )?,
        render_watch_frame(
            "watch-compact-normal",
            "Watch Compact Normal",
            72,
            24,
            ctx,
            scratch_dir,
        )?,
    ];

    for fixture in liveliness_frame_fixtures(ctx) {
        frames.push(render_liveliness_watch_frame(ctx, scratch_dir, fixture)?);
    }

    for fixture in day_context_frame_fixtures(ctx) {
        frames.push(render_day_context_watch_frame(ctx, scratch_dir, fixture)?);
    }

    Ok(frames)
}

pub(crate) struct WatchFrameFixture<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub width: u16,
    pub height: u16,
    pub state: &'a PetState,
    pub now: OffsetDateTime,
}

#[derive(Clone)]
pub(crate) struct WatchLifeFixture {
    pub profile: PetLifeProfile,
    pub color_capability: ColorCapability,
}

struct LivelinessFrameFixture {
    id: &'static str,
    title: &'static str,
    width: u16,
    height: u16,
    now: OffsetDateTime,
    life: WatchLifeFixture,
}

struct DayContextFrameFixture {
    id: &'static str,
    title: &'static str,
    width: u16,
    height: u16,
    now: OffsetDateTime,
    state: fn(&PreviewRenderContext) -> PetState,
    life: WatchLifeFixture,
    day_context: DayContext,
    hold_eyes_closed: bool,
}

fn render_watch_frame(
    id: &str,
    title: &str,
    width: u16,
    height: u16,
    ctx: &PreviewRenderContext,
    scratch_dir: &Path,
) -> Result<PreviewFrame> {
    let state = seeded_pet_state(ctx);
    render_watch_frame_from_state(
        ctx,
        scratch_dir,
        WatchFrameFixture {
            id,
            title,
            width,
            height,
            state: &state,
            now: ctx.fixed_now,
        },
    )
}

fn render_liveliness_watch_frame(
    ctx: &PreviewRenderContext,
    scratch_dir: &Path,
    fixture: LivelinessFrameFixture,
) -> Result<PreviewFrame> {
    let state = liveliness_pet_state(ctx);
    render_watch_frame_from_state_with_life(
        ctx,
        scratch_dir,
        WatchFrameFixture {
            id: fixture.id,
            title: fixture.title,
            width: fixture.width,
            height: fixture.height,
            state: &state,
            now: fixture.now,
        },
        Some(&fixture.life),
        None,
        false,
    )
}

fn render_day_context_watch_frame(
    ctx: &PreviewRenderContext,
    scratch_dir: &Path,
    fixture: DayContextFrameFixture,
) -> Result<PreviewFrame> {
    let state = (fixture.state)(ctx);
    render_watch_frame_from_state_with_life(
        ctx,
        scratch_dir,
        WatchFrameFixture {
            id: fixture.id,
            title: fixture.title,
            width: fixture.width,
            height: fixture.height,
            state: &state,
            now: fixture.now,
        },
        Some(&fixture.life),
        Some(fixture.day_context),
        fixture.hold_eyes_closed,
    )
}

pub(crate) fn render_watch_frame_from_state(
    ctx: &PreviewRenderContext,
    scratch_dir: &Path,
    fixture: WatchFrameFixture<'_>,
) -> Result<PreviewFrame> {
    render_watch_frame_from_state_with_life(ctx, scratch_dir, fixture, None, None, false)
}

fn render_watch_frame_from_state_with_life(
    ctx: &PreviewRenderContext,
    scratch_dir: &Path,
    fixture: WatchFrameFixture<'_>,
    life: Option<&WatchLifeFixture>,
    day_context: Option<DayContext>,
    hold_eyes_closed: bool,
) -> Result<PreviewFrame> {
    let WatchFrameFixture {
        id,
        title,
        width,
        height,
        state,
        now,
    } = fixture;
    let usage_path = scratch_dir.join(format!("{id}.sqlite"));
    seed_usage_store(&usage_path, now)?;
    let color_capability = life.map_or(ctx.render.color_capability, |life| life.color_capability);
    let render = RenderContext::with_clock(color_capability, WatchClock::fixed(now));
    let mut vm = build_watch_view_model_at(
        state,
        &usage_path,
        now,
        crate::storage::day_axis::LocalDayMapper::Fixed(UtcOffset::UTC),
    )?;
    if let Some(life) = life {
        vm.life_profile = life.profile.clone();
    }
    if let Some(day) = day_context {
        vm.day_context = day;
        vm.life_profile.calm_mode = day.asleep;
        vm.current_speech = crate::pet::speech::current_pet_speech_for_scene(
            vm.pet_render.mood,
            &vm.life_profile,
            &day,
            now,
        );
    }
    if hold_eyes_closed || day_context.is_some_and(|day| day.tiredness > 0.0) {
        rerender_pet_for_view_model(
            &mut vm,
            now.unix_timestamp().max(0) as u64,
            hold_eyes_closed,
        )?;
    }
    let layout = layout_watch_with_context(Rect::new(0, 0, width, height), &vm, &render);

    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| {
        render_watch_frame_with_layout(frame, &vm, &render, &layout);
    })?;

    let mut frame = frame_from_buffer(id, title, terminal.backend().buffer());
    frame.layout = Some(preview_layout(id, &layout));
    Ok(frame)
}

fn liveliness_frame_fixtures(ctx: &PreviewRenderContext) -> Vec<LivelinessFrameFixture> {
    let dawn = ctx.fixed_now;
    let midday = ctx.fixed_now + Duration::hours(4);
    let evening = ctx.fixed_now + Duration::hours(10);

    vec![
        LivelinessFrameFixture {
            id: "watch-liveliness-s6-idle-dawn",
            title: "Watch Liveliness S6 Idle Dawn",
            width: 120,
            height: 32,
            now: dawn,
            life: WatchLifeFixture {
                profile: idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
        },
        LivelinessFrameFixture {
            id: "watch-liveliness-s6-warm-midday",
            title: "Watch Liveliness S6 Warm Midday",
            width: 120,
            height: 32,
            now: midday,
            life: WatchLifeFixture {
                profile: warm_life_profile(false),
                color_capability: ColorCapability::Truecolor,
            },
        },
        LivelinessFrameFixture {
            id: "watch-liveliness-s6-hot-midday",
            title: "Watch Liveliness S6 Hot Midday",
            width: 120,
            height: 32,
            now: midday,
            life: WatchLifeFixture {
                profile: hot_life_profile(false),
                color_capability: ColorCapability::Truecolor,
            },
        },
        LivelinessFrameFixture {
            id: "watch-liveliness-s6-cooling-evening",
            title: "Watch Liveliness S6 Cooling Evening",
            width: 120,
            height: 32,
            now: evening,
            life: WatchLifeFixture {
                profile: cooling_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
        },
        LivelinessFrameFixture {
            id: "watch-liveliness-compact-s6-hot",
            title: "Watch Liveliness Compact S6 Hot",
            width: 72,
            height: 24,
            now: midday,
            life: WatchLifeFixture {
                profile: hot_life_profile(false),
                color_capability: ColorCapability::Truecolor,
            },
        },
        LivelinessFrameFixture {
            id: "watch-liveliness-flat-s6-hot",
            title: "Watch Liveliness Flat S6 Hot",
            width: 120,
            height: 32,
            now: midday,
            life: WatchLifeFixture {
                profile: hot_life_profile(false),
                color_capability: ColorCapability::Flat,
            },
        },
        LivelinessFrameFixture {
            id: "watch-liveliness-calm-mode-s6-hot",
            title: "Watch Liveliness Calm Mode S6 Hot",
            width: 120,
            height: 32,
            now: midday,
            life: WatchLifeFixture {
                profile: hot_life_profile(true),
                color_capability: ColorCapability::Truecolor,
            },
        },
    ]
}

fn day_context_frame_fixtures(ctx: &PreviewRenderContext) -> Vec<DayContextFrameFixture> {
    let fixed_now = ctx.fixed_now;
    let speech_visible_now = fixed_now + Duration::seconds(10);
    vec![
        DayContextFrameFixture {
            id: "watch-daycontext-night-asleep",
            title: "Watch DayContext Night Asleep",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: calm_idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: night_asleep_day_context(fixed_now),
            hold_eyes_closed: true,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-dawn-crossing",
            title: "Watch DayContext Dawn Crossing",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: warm_life_profile(false),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: dawn_crossing_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-night-wake-catchup",
            title: "Watch DayContext Night Wake Catchup",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: cooling_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: night_wake_catchup_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-hatch-at-night",
            title: "Watch DayContext Hatch At Night",
            width: 120,
            height: 32,
            now: fixed_now,
            state: newborn_pet_state,
            life: WatchLifeFixture {
                profile: idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: night_newborn_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-dream-night",
            title: "Watch DayContext Dream Night",
            width: 120,
            height: 32,
            now: speech_visible_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: calm_idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: dream_night_day_context(speech_visible_now),
            hold_eyes_closed: true,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-heavy-day-evening",
            title: "Watch DayContext Heavy Day Evening",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: cooling_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: heavy_day_evening_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-light-day-morning",
            title: "Watch DayContext Light Day Morning",
            width: 120,
            height: 32,
            now: speech_visible_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: light_day_morning_day_context(speech_visible_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-weekend-midday",
            title: "Watch DayContext Weekend Midday",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: weekend_midday_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-climate-cache-week",
            title: "Watch DayContext Climate Cache Week",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: climate_cache_week_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-prop-resonance-planter",
            title: "Watch DayContext Prop Resonance Planter",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: prop_resonance_planter_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-midnight-mid-session",
            title: "Watch DayContext Midnight Mid Session",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: warm_life_profile(false),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: midnight_mid_session_day_context(fixed_now),
            hold_eyes_closed: false,
        },
    ]
}

fn seeded_pet_state(ctx: &PreviewRenderContext) -> PetState {
    let mut state = PetState::new_for_test("glorp-preview-watch", "Mochi");
    state.pet.generated_species = Species::Fuzz;
    state.stage = Stage::S4;
    state.xp = 8.5;
    state.lifetime_effective_tokens = 125_000.0;
    state.vitals = Vitals {
        fed: 70.0,
        happiness: 72.0,
        energy: 68.0,
    };
    state.created_at = ctx.fixed_now - Duration::days(18);
    state.last_updated_at = ctx.fixed_now;
    state.last_usage_poll_at = Some(ctx.fixed_now - Duration::minutes(5));
    // Hand-seed a few narrative entries for visual review. In production these
    // are auto-generated by the narration system; here we demonstrate the feed
    // layout with a fixed set so the snapshot is stable and reviewable.
    state.recent_events = vec![
        NarrativeEvent {
            observed_at: ctx.fixed_now - Duration::minutes(15),
            text: format!("{} brightened", state.pet.accepted_name),
        },
        NarrativeEvent {
            observed_at: ctx.fixed_now - Duration::minutes(10),
            text: format!("{} munched", state.pet.accepted_name),
        },
        NarrativeEvent {
            observed_at: ctx.fixed_now - Duration::minutes(2),
            text: format!(
                "{} evolved into {}",
                state.pet.accepted_name,
                crate::pet::art::stage_label(state.pet.generated_species, state.stage)
            ),
        },
    ];
    state.habitat.earned_props = vec![
        EarnedHabitatProp {
            id: HabitatPropId::new("codex_signal_lamp"),
            earned_at: ctx.fixed_now - Duration::days(12),
            source: HabitatPropSource::ProviderFirstUse {
                provider_surface: "codex".to_string(),
            },
        },
        EarnedHabitatProp {
            id: HabitatPropId::new("heavy_session_planter"),
            earned_at: ctx.fixed_now - Duration::days(6),
            source: HabitatPropSource::HeavySession,
        },
        EarnedHabitatProp {
            id: HabitatPropId::new("token_pebble_25k"),
            earned_at: ctx.fixed_now - Duration::days(10),
            source: HabitatPropSource::LifetimeTokens {
                threshold: 25_000.0,
            },
        },
        EarnedHabitatProp {
            id: HabitatPropId::new("token_shell_100k"),
            earned_at: ctx.fixed_now - Duration::days(4),
            source: HabitatPropSource::LifetimeTokens {
                threshold: 100_000.0,
            },
        },
    ];
    state
}

fn liveliness_pet_state(ctx: &PreviewRenderContext) -> PetState {
    let mut state = seeded_pet_state(ctx);
    state.pet.generated_species = Species::Crystal;
    state.stage = Stage::S6;
    state.xp = 72.0;
    state.lifetime_effective_tokens = 2_400_000.0;
    state.vitals = Vitals {
        fed: 86.0,
        happiness: 88.0,
        energy: 84.0,
    };
    state.created_at = ctx.fixed_now - Duration::days(96);
    state.last_updated_at = ctx.fixed_now;
    state.last_usage_poll_at = Some(ctx.fixed_now - Duration::minutes(1));
    state.recent_events = vec![
        NarrativeEvent {
            observed_at: ctx.fixed_now - Duration::minutes(14),
            text: format!("{} caught a warm signal", state.pet.accepted_name),
        },
        NarrativeEvent {
            observed_at: ctx.fixed_now - Duration::minutes(5),
            text: format!("{} polished the crystal den", state.pet.accepted_name),
        },
        NarrativeEvent {
            observed_at: ctx.fixed_now - Duration::minutes(1),
            text: format!(
                "{} evolved into {}",
                state.pet.accepted_name,
                crate::pet::art::stage_label(state.pet.generated_species, state.stage)
            ),
        },
    ];
    state
}

fn idle_life_profile() -> PetLifeProfile {
    PetLifeProfile {
        activity_level: 0.0,
        burst_level: 0.0,
        source_accent: None,
        work_weather: WorkWeather::Clear,
        prop_reactions: Vec::new(),
        idle: IdleLifeState {
            idle_minutes: 45,
            is_recently_active: false,
        },
        calm_mode: false,
    }
}

fn warm_life_profile(calm_mode: bool) -> PetLifeProfile {
    PetLifeProfile {
        activity_level: 0.68,
        burst_level: 0.24,
        source_accent: Some(SourceAccent::Balanced),
        work_weather: WorkWeather::Mixed,
        prop_reactions: vec![PropReaction {
            prop_id: HabitatPropId::new("codex_signal_lamp"),
            intensity: 0.35,
            kind: PropReactionKind::Glow,
        }],
        idle: IdleLifeState {
            idle_minutes: 0,
            is_recently_active: true,
        },
        calm_mode,
    }
}

fn hot_life_profile(calm_mode: bool) -> PetLifeProfile {
    PetLifeProfile {
        activity_level: 1.56,
        burst_level: 1.22,
        source_accent: Some(SourceAccent::Codex),
        work_weather: WorkWeather::OutputSparks,
        prop_reactions: vec![
            PropReaction {
                prop_id: HabitatPropId::new("codex_signal_lamp"),
                intensity: 0.9,
                kind: PropReactionKind::Pulse,
            },
            PropReaction {
                prop_id: HabitatPropId::new("heavy_session_planter"),
                intensity: 0.72,
                kind: PropReactionKind::Bloom,
            },
        ],
        idle: IdleLifeState {
            idle_minutes: 0,
            is_recently_active: true,
        },
        calm_mode,
    }
}

fn cooling_life_profile() -> PetLifeProfile {
    PetLifeProfile {
        activity_level: 0.38,
        burst_level: 0.0,
        source_accent: Some(SourceAccent::Claude),
        work_weather: WorkWeather::CacheMist,
        prop_reactions: vec![PropReaction {
            prop_id: HabitatPropId::new("token_shell_100k"),
            intensity: 0.28,
            kind: PropReactionKind::Glow,
        }],
        idle: IdleLifeState {
            idle_minutes: 12,
            is_recently_active: false,
        },
        calm_mode: false,
    }
}

fn calm_idle_life_profile() -> PetLifeProfile {
    PetLifeProfile {
        activity_level: 0.0,
        burst_level: 0.0,
        source_accent: None,
        work_weather: WorkWeather::Clear,
        prop_reactions: Vec::new(),
        idle: IdleLifeState {
            idle_minutes: 45,
            is_recently_active: false,
        },
        calm_mode: true,
    }
}

fn newborn_pet_state(ctx: &PreviewRenderContext) -> PetState {
    let mut state = PetState::new_for_test("glorp-preview-watch", "Mochi");
    state.pet.generated_species = Species::Fuzz;
    state.stage = Stage::S0;
    state.xp = 0.0;
    state.lifetime_effective_tokens = 0.0;
    state.vitals = Vitals {
        fed: 80.0,
        happiness: 80.0,
        energy: 80.0,
    };
    state.created_at = ctx.fixed_now;
    state.last_updated_at = ctx.fixed_now;
    state.last_usage_poll_at = None;
    state.recent_events = Vec::new();
    state.habitat.earned_props = Vec::new();
    state
}

fn night_asleep_day_context(fixed_now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Night,
        phase_started_at_utc: fixed_now - Duration::hours(2),
        phase_ends_at_utc: fixed_now + Duration::hours(6),
        asleep: true,
        sleep_onset_utc: Some(fixed_now - Duration::minutes(25)),
        ..DayContext::default()
    }
}

fn dawn_crossing_day_context(fixed_now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Dawn,
        phase_started_at_utc: fixed_now - Duration::minutes(10),
        phase_ends_at_utc: fixed_now + Duration::minutes(20),
        asleep: false,
        ..DayContext::default()
    }
}

fn night_wake_catchup_day_context(fixed_now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Night,
        phase_started_at_utc: fixed_now - Duration::hours(2),
        phase_ends_at_utc: fixed_now + Duration::hours(6),
        asleep: false,
        wake_resume: Some(WakeResume {
            from_eval_utc: fixed_now - Duration::hours(3),
            woke_at_utc: fixed_now - Duration::minutes(2),
        }),
        ..DayContext::default()
    }
}

fn night_newborn_day_context(fixed_now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Night,
        phase_started_at_utc: fixed_now - Duration::hours(2),
        phase_ends_at_utc: fixed_now + Duration::hours(6),
        asleep: false,
        ..DayContext::default()
    }
}

fn dream_night_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Night,
        phase_started_at_utc: now - Duration::hours(3),
        phase_ends_at_utc: now + Duration::hours(5),
        asleep: true,
        sleep_onset_utc: Some(now - Duration::hours(1)),
        yesterday: Some(DaySummary {
            ratio: 1.6,
            dominant_shape: Some(WorkWeather::CacheMist),
        }),
        date_seed: 7,
        mature: true,
        local_day_started_utc: now - Duration::hours(2),
        local_day_rollover_utc: now + Duration::hours(22),
        ..DayContext::default()
    }
}

fn heavy_day_evening_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Dusk,
        phase_started_at_utc: now - Duration::hours(1),
        phase_ends_at_utc: now + Duration::hours(2),
        today_ratio: 1.7,
        tiredness: 0.85,
        mature: true,
        local_day_started_utc: now - Duration::hours(13),
        local_day_rollover_utc: now + Duration::hours(11),
        ..DayContext::default()
    }
}

fn light_day_morning_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Dawn,
        phase_started_at_utc: now - Duration::minutes(40),
        phase_ends_at_utc: now + Duration::minutes(80),
        today_ratio: 0.02,
        yesterday: Some(DaySummary {
            ratio: 0.04,
            dominant_shape: None,
        }),
        mature: true,
        local_day_started_utc: now - Duration::hours(7),
        local_day_rollover_utc: now + Duration::hours(17),
        ..DayContext::default()
    }
}

fn weekend_midday_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Day,
        phase_started_at_utc: now - Duration::hours(3),
        phase_ends_at_utc: now + Duration::hours(5),
        is_weekend: true,
        weekend_share: 0.05,
        today_ratio: 0.3,
        mature: true,
        local_day_started_utc: now - Duration::hours(12),
        local_day_rollover_utc: now + Duration::hours(12),
        ..DayContext::default()
    }
}

fn climate_cache_week_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Day,
        phase_started_at_utc: now - Duration::hours(3),
        phase_ends_at_utc: now + Duration::hours(5),
        climate: Some(WorkWeather::CacheMist),
        today_ratio: 0.5,
        mature: true,
        local_day_started_utc: now - Duration::hours(12),
        local_day_rollover_utc: now + Duration::hours(12),
        ..DayContext::default()
    }
}

fn prop_resonance_planter_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Day,
        phase_started_at_utc: now - Duration::hours(3),
        phase_ends_at_utc: now + Duration::hours(5),
        yesterday: Some(DaySummary {
            ratio: 1.9,
            dominant_shape: Some(WorkWeather::OutputSparks),
        }),
        today_ratio: 0.4,
        mature: true,
        local_day_started_utc: now - Duration::hours(12),
        local_day_rollover_utc: now + Duration::hours(12),
        ..DayContext::default()
    }
}

fn midnight_mid_session_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Night,
        phase_started_at_utc: now - Duration::hours(2),
        phase_ends_at_utc: now + Duration::hours(6),
        asleep: false,
        today_ratio: 0.03,
        tiredness: 0.45,
        yesterday: Some(DaySummary {
            ratio: 1.3,
            dominant_shape: Some(WorkWeather::Mixed),
        }),
        mature: true,
        local_day_started_utc: now - Duration::minutes(10),
        local_day_rollover_utc: now + Duration::hours(24) - Duration::minutes(10),
        ..DayContext::default()
    }
}

fn seed_usage_store(path: &Path, now: OffsetDateTime) -> Result<()> {
    let mut usage = UsageStore::open(path)?;
    for (surface, observed_at, effective_tokens, model) in [
        (
            "claude-code",
            now - Duration::minutes(5),
            12_500.0,
            "claude-sonnet",
        ),
        ("codex", now - Duration::minutes(8), 4_200.0, "gpt-5-codex"),
        (
            "claude-code",
            now - Duration::days(1),
            8_800.0,
            "claude-sonnet",
        ),
    ] {
        usage.insert_event(&NormalizedUsageEvent {
            provider_surface: surface.to_string(),
            model: Some(model.to_string()),
            provider_delta_id: Some(format!("preview-{surface}-{effective_tokens}")),
            ..NormalizedUsageEvent::for_test_at(observed_at, effective_tokens)
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_frames_include_wide_tall_wide_and_compact() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = PreviewRenderContext::deterministic();

        let frames = watch_frames(&ctx, dir.path()).unwrap();

        assert_eq!(frames.len(), 21);
        assert_eq!(frames[0].id, "watch-wide-normal");
        assert_eq!((frames[0].width, frames[0].height), (120, 32));
        assert_eq!(frames[1].id, "watch-tall-wide");
        assert_eq!((frames[1].width, frames[1].height), (180, 50));
        assert_eq!(frames[2].id, "watch-compact-normal");
        assert_eq!((frames[2].width, frames[2].height), (72, 24));
        assert_eq!(frames[3].id, "watch-liveliness-s6-idle-dawn");
        assert_eq!((frames[3].width, frames[3].height), (120, 32));
        assert_eq!(frames[4].id, "watch-liveliness-s6-warm-midday");
        assert_eq!(frames[5].id, "watch-liveliness-s6-hot-midday");
        assert_eq!(frames[6].id, "watch-liveliness-s6-cooling-evening");
        assert_eq!(frames[7].id, "watch-liveliness-compact-s6-hot");
        assert_eq!((frames[7].width, frames[7].height), (72, 24));
        assert_eq!(frames[8].id, "watch-liveliness-flat-s6-hot");
        assert_eq!(frames[9].id, "watch-liveliness-calm-mode-s6-hot");
        assert_eq!(frames[10].id, "watch-daycontext-night-asleep");
        assert_eq!((frames[10].width, frames[10].height), (120, 32));
        assert_eq!(frames[11].id, "watch-daycontext-dawn-crossing");
        assert_eq!((frames[11].width, frames[11].height), (120, 32));
        assert_eq!(frames[12].id, "watch-daycontext-night-wake-catchup");
        assert_eq!((frames[12].width, frames[12].height), (120, 32));
        assert_eq!(frames[13].id, "watch-daycontext-hatch-at-night");
        assert_eq!((frames[13].width, frames[13].height), (120, 32));
        assert_eq!(frames[14].id, "watch-daycontext-dream-night");
        assert_eq!((frames[14].width, frames[14].height), (120, 32));
        assert_eq!(frames[15].id, "watch-daycontext-heavy-day-evening");
        assert_eq!((frames[15].width, frames[15].height), (120, 32));
        assert_eq!(frames[16].id, "watch-daycontext-light-day-morning");
        assert_eq!((frames[16].width, frames[16].height), (120, 32));
        assert_eq!(frames[17].id, "watch-daycontext-weekend-midday");
        assert_eq!((frames[17].width, frames[17].height), (120, 32));
        assert_eq!(frames[18].id, "watch-daycontext-climate-cache-week");
        assert_eq!((frames[18].width, frames[18].height), (120, 32));
        assert_eq!(frames[19].id, "watch-daycontext-prop-resonance-planter");
        assert_eq!((frames[19].width, frames[19].height), (120, 32));
        assert_eq!(frames[20].id, "watch-daycontext-midnight-mid-session");
        assert_eq!((frames[20].width, frames[20].height), (120, 32));
    }

    #[test]
    fn watch_frames_are_stable_for_fixed_time() {
        let first_dir = tempfile::tempdir().unwrap();
        let second_dir = tempfile::tempdir().unwrap();
        let ctx = PreviewRenderContext::deterministic();

        let first = watch_frames(&ctx, first_dir.path()).unwrap();
        let second = watch_frames(&ctx, second_dir.path()).unwrap();

        assert_eq!(first, second);
    }
}
