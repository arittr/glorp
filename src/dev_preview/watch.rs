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
use serde_json::{json, Value};
use std::collections::BTreeMap;
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

    for fixture in alive_room_frame_fixtures(ctx) {
        frames.push(render_alive_room_watch_frame(ctx, scratch_dir, fixture)?);
    }

    for fixture in species_dialect_frame_fixtures(ctx) {
        frames.push(render_alive_room_watch_frame(ctx, scratch_dir, fixture)?);
    }

    frames.push(render_activity_identity_watch_frame(
        ctx,
        scratch_dir,
        "watch-activity-identity-ensemble",
        "Watch Activity Identity Ensemble",
        120,
        32,
        seed_usage_store_ensemble,
    )?);

    frames.push(render_activity_identity_watch_frame(
        ctx,
        scratch_dir,
        "watch-activity-identity-unknown",
        "Watch Activity Identity Unknown Source",
        120,
        32,
        seed_usage_store_unknown_source,
    )?);

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
    pub last_feed_pulse_at: Option<OffsetDateTime>,
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
        seed_usage_store,
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
        seed_usage_store,
    )
}

pub(crate) fn render_watch_frame_from_state(
    ctx: &PreviewRenderContext,
    scratch_dir: &Path,
    fixture: WatchFrameFixture<'_>,
) -> Result<PreviewFrame> {
    render_watch_frame_from_state_with_life(
        ctx,
        scratch_dir,
        fixture,
        None,
        None,
        false,
        seed_usage_store,
    )
}

fn render_watch_frame_from_state_with_life(
    ctx: &PreviewRenderContext,
    scratch_dir: &Path,
    fixture: WatchFrameFixture<'_>,
    life: Option<&WatchLifeFixture>,
    day_context: Option<DayContext>,
    hold_eyes_closed: bool,
    seed_usage: fn(&Path, OffsetDateTime) -> Result<()>,
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
    seed_usage(&usage_path, now)?;
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
        vm.last_feed_pulse_at = life.last_feed_pulse_at;
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
    frame.contract.scene = Some(
        crate::dev_preview::contract::PreviewSceneArtifact::from_watch_view_model(
            id,
            &vm,
            now,
            frame.layout.as_ref(),
        ),
    );
    Ok(frame)
}

fn render_activity_identity_watch_frame(
    ctx: &PreviewRenderContext,
    scratch_dir: &Path,
    id: &str,
    title: &str,
    width: u16,
    height: u16,
    seed_usage: fn(&Path, OffsetDateTime) -> Result<()>,
) -> Result<PreviewFrame> {
    let state = seeded_pet_state(ctx);
    render_watch_frame_from_state_with_life(
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
        None,
        None,
        false,
        seed_usage,
    )
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
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
                last_feed_pulse_at: None,
            },
            day_context: midnight_mid_session_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-dawn-fresh",
            title: "Watch DayContext Dawn Fresh",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: cooling_life_profile(),
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: dawn_fresh_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-dusk-heavy",
            title: "Watch DayContext Dusk Heavy",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: cooling_life_profile(),
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: dusk_heavy_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-night-quiet",
            title: "Watch DayContext Night Quiet",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: night_quiet_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-work-output-sparks",
            title: "Watch DayContext Work Output Sparks",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: work_life_profile(WorkWeather::OutputSparks),
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: work_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-work-reasoning-pulse",
            title: "Watch DayContext Work Reasoning Pulse",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: work_life_profile(WorkWeather::ReasoningPulse),
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: work_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-work-cache-mist",
            title: "Watch DayContext Work Cache Mist",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: work_life_profile(WorkWeather::CacheMist),
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: work_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-work-mixed",
            title: "Watch DayContext Work Mixed",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: work_life_profile(WorkWeather::Mixed),
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: work_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-work-clear",
            title: "Watch DayContext Work Clear",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: work_life_profile(WorkWeather::Clear),
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: work_day_context(fixed_now),
            hold_eyes_closed: false,
        },
    ]
}

struct AliveRoomFrameFixture {
    id: &'static str,
    title: &'static str,
    width: u16,
    height: u16,
    species: Species,
    stage: Stage,
    props: Vec<EarnedHabitatProp>,
    life: WatchLifeFixture,
    day_context: DayContext,
    expected_biome: &'static str,
    expected_emitter: Option<&'static str>,
}

fn alive_room_frame_fixtures(ctx: &PreviewRenderContext) -> Vec<AliveRoomFrameFixture> {
    let fixed_now = ctx.fixed_now;
    vec![
        AliveRoomFrameFixture {
            id: "room-starter-day-clear",
            title: "Room Starter Day Clear",
            width: 120,
            height: 32,
            species: Species::Fuzz,
            stage: Stage::S0,
            props: vec![],
            life: WatchLifeFixture {
                profile: idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: DayContext {
                day_phase: DayPhase::Day,
                mature: false,
                ..DayContext::default()
            },
            expected_biome: "Starter",
            expected_emitter: None,
        },
        AliveRoomFrameFixture {
            id: "room-botanical-cache-evening",
            title: "Room Botanical Cache Evening",
            width: 120,
            height: 32,
            species: Species::Crystal,
            stage: Stage::S6,
            props: vec![
                EarnedHabitatProp {
                    id: HabitatPropId::new("heavy_session_planter"),
                    earned_at: fixed_now - Duration::days(20),
                    source: HabitatPropSource::HeavySession,
                },
                EarnedHabitatProp {
                    id: HabitatPropId::new("token_moss_tuft_250k"),
                    earned_at: fixed_now - Duration::days(15),
                    source: HabitatPropSource::LifetimeTokens {
                        threshold: 250_000.0,
                    },
                },
            ],
            life: WatchLifeFixture {
                profile: {
                    let mut p = cooling_life_profile();
                    p.prop_reactions = vec![PropReaction {
                        prop_id: HabitatPropId::new("heavy_session_planter"),
                        intensity: 0.5,
                        kind: PropReactionKind::Glow,
                    }];
                    p
                },
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: DayContext {
                day_phase: DayPhase::Dusk,
                today_ratio: 0.8,
                tiredness: 0.3,
                mature: true,
                ..DayContext::default()
            },
            expected_biome: "Botanical",
            expected_emitter: Some("heavy_session_planter"),
        },
        AliveRoomFrameFixture {
            id: "room-technical-output-active",
            title: "Room Technical Output Active",
            width: 120,
            height: 32,
            species: Species::Crystal,
            stage: Stage::S6,
            props: vec![EarnedHabitatProp {
                id: HabitatPropId::new("codex_signal_lamp"),
                earned_at: fixed_now - Duration::days(12),
                source: HabitatPropSource::ProviderFirstUse {
                    provider_surface: "codex".to_string(),
                },
            }],
            life: WatchLifeFixture {
                profile: hot_life_profile(false),
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: Some(fixed_now - Duration::seconds(5)),
            },
            day_context: DayContext {
                day_phase: DayPhase::Day,
                mature: true,
                ..DayContext::default()
            },
            expected_biome: "Technical",
            expected_emitter: Some("codex_signal_lamp"),
        },
        AliveRoomFrameFixture {
            id: "room-celestial-artifact-night",
            title: "Room Celestial Artifact Night",
            width: 120,
            height: 32,
            species: Species::Crystal,
            stage: Stage::S6,
            props: vec![
                EarnedHabitatProp {
                    id: HabitatPropId::new("token_lantern_10m"),
                    earned_at: fixed_now - Duration::days(8),
                    source: HabitatPropSource::LifetimeTokens {
                        threshold: 10_000_000.0,
                    },
                },
                EarnedHabitatProp {
                    id: HabitatPropId::new("token_shell_100k"),
                    earned_at: fixed_now - Duration::days(10),
                    source: HabitatPropSource::LifetimeTokens {
                        threshold: 100_000.0,
                    },
                },
            ],
            life: WatchLifeFixture {
                profile: {
                    let mut p = calm_idle_life_profile();
                    p.prop_reactions = vec![PropReaction {
                        prop_id: HabitatPropId::new("token_lantern_10m"),
                        intensity: 0.6,
                        kind: PropReactionKind::Glow,
                    }];
                    p
                },
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: DayContext {
                day_phase: DayPhase::Night,
                asleep: true,
                mature: true,
                ..DayContext::default()
            },
            expected_biome: "Celestial",
            expected_emitter: Some("token_lantern_10m"),
        },
        AliveRoomFrameFixture {
            id: "room-cozy-weekend-quiet",
            title: "Room Cozy Weekend Quiet",
            width: 120,
            height: 32,
            species: Species::Fuzz,
            stage: Stage::S4,
            props: vec![
                EarnedHabitatProp {
                    id: HabitatPropId::new("wilt_recovery_sprout"),
                    earned_at: fixed_now - Duration::days(5),
                    source: HabitatPropSource::WiltRecovery,
                },
                EarnedHabitatProp {
                    id: HabitatPropId::new("token_lantern_10m"),
                    earned_at: fixed_now - Duration::days(7),
                    source: HabitatPropSource::LifetimeTokens {
                        threshold: 10_000_000.0,
                    },
                },
                EarnedHabitatProp {
                    id: HabitatPropId::new("token_moss_tuft_250k"),
                    earned_at: fixed_now - Duration::days(9),
                    source: HabitatPropSource::LifetimeTokens {
                        threshold: 250_000.0,
                    },
                },
            ],
            life: WatchLifeFixture {
                profile: idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: DayContext {
                day_phase: DayPhase::Day,
                is_weekend: true,
                weekend_share: 0.05,
                mature: true,
                ..DayContext::default()
            },
            expected_biome: "Cozy",
            expected_emitter: None,
        },
        AliveRoomFrameFixture {
            id: "room-mixed-full-wide",
            title: "Room Mixed Full Wide",
            width: 180,
            height: 50,
            species: Species::Crystal,
            stage: Stage::S6,
            props: vec![
                EarnedHabitatProp {
                    id: HabitatPropId::new("heavy_session_planter"),
                    earned_at: fixed_now - Duration::days(18),
                    source: HabitatPropSource::HeavySession,
                },
                EarnedHabitatProp {
                    id: HabitatPropId::new("codex_signal_lamp"),
                    earned_at: fixed_now - Duration::days(14),
                    source: HabitatPropSource::ProviderFirstUse {
                        provider_surface: "codex".to_string(),
                    },
                },
                EarnedHabitatProp {
                    id: HabitatPropId::new("token_shard_1m"),
                    earned_at: fixed_now - Duration::days(11),
                    source: HabitatPropSource::LifetimeTokens {
                        threshold: 1_000_000.0,
                    },
                },
                EarnedHabitatProp {
                    id: HabitatPropId::new("token_treasure_chest_2m"),
                    earned_at: fixed_now - Duration::days(10),
                    source: HabitatPropSource::LifetimeTokens {
                        threshold: 2_000_000.0,
                    },
                },
                EarnedHabitatProp {
                    id: HabitatPropId::new("token_orbit_5m"),
                    earned_at: fixed_now - Duration::days(9),
                    source: HabitatPropSource::LifetimeTokens {
                        threshold: 5_000_000.0,
                    },
                },
                EarnedHabitatProp {
                    id: HabitatPropId::new("token_spark_500k"),
                    earned_at: fixed_now - Duration::days(8),
                    source: HabitatPropSource::LifetimeTokens {
                        threshold: 500_000.0,
                    },
                },
                EarnedHabitatProp {
                    id: HabitatPropId::new("token_friendly_cloud_750k"),
                    earned_at: fixed_now - Duration::days(7),
                    source: HabitatPropSource::LifetimeTokens {
                        threshold: 750_000.0,
                    },
                },
                EarnedHabitatProp {
                    id: HabitatPropId::new("token_lantern_10m"),
                    earned_at: fixed_now - Duration::days(6),
                    source: HabitatPropSource::LifetimeTokens {
                        threshold: 10_000_000.0,
                    },
                },
                EarnedHabitatProp {
                    id: HabitatPropId::new("token_hanging_vine_25m"),
                    earned_at: fixed_now - Duration::days(5),
                    source: HabitatPropSource::LifetimeTokens {
                        threshold: 25_000_000.0,
                    },
                },
                EarnedHabitatProp {
                    id: HabitatPropId::new("token_shell_100k"),
                    earned_at: fixed_now - Duration::days(4),
                    source: HabitatPropSource::LifetimeTokens {
                        threshold: 100_000.0,
                    },
                },
                EarnedHabitatProp {
                    id: HabitatPropId::new("token_moss_tuft_250k"),
                    earned_at: fixed_now - Duration::days(3),
                    source: HabitatPropSource::LifetimeTokens {
                        threshold: 250_000.0,
                    },
                },
            ],
            life: WatchLifeFixture {
                profile: {
                    let mut p = hot_life_profile(false);
                    p.work_weather = WorkWeather::Mixed;
                    p.prop_reactions = vec![PropReaction {
                        prop_id: HabitatPropId::new("codex_signal_lamp"),
                        intensity: 0.9,
                        kind: PropReactionKind::Pulse,
                    }];
                    p
                },
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: Some(fixed_now - Duration::seconds(5)),
            },
            day_context: DayContext {
                day_phase: DayPhase::Day,
                mature: true,
                ..DayContext::default()
            },
            expected_biome: "Cozy",
            expected_emitter: Some("codex_signal_lamp"),
        },
        AliveRoomFrameFixture {
            id: "room-heavy-day-cozy-large",
            title: "Room Heavy Day Cozy Large",
            width: 120,
            height: 32,
            species: Species::Crystal,
            stage: Stage::S6,
            props: vec![EarnedHabitatProp {
                id: HabitatPropId::new("heavy_session_planter"),
                earned_at: fixed_now - Duration::days(6),
                source: HabitatPropSource::HeavySession,
            }],
            life: WatchLifeFixture {
                profile: {
                    let mut p = cooling_life_profile();
                    p.prop_reactions = vec![PropReaction {
                        prop_id: HabitatPropId::new("heavy_session_planter"),
                        intensity: 0.5,
                        kind: PropReactionKind::Glow,
                    }];
                    p
                },
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: DayContext {
                day_phase: DayPhase::Dusk,
                today_ratio: 1.7,
                tiredness: 0.85,
                mature: true,
                ..DayContext::default()
            },
            expected_biome: "Botanical",
            expected_emitter: Some("heavy_session_planter"),
        },
        AliveRoomFrameFixture {
            id: "room-dawn-wake-small",
            title: "Room Dawn Wake Small",
            width: 72,
            height: 24,
            species: Species::Crystal,
            stage: Stage::S6,
            props: vec![EarnedHabitatProp {
                id: HabitatPropId::new("token_lantern_10m"),
                earned_at: fixed_now - Duration::days(4),
                source: HabitatPropSource::LifetimeTokens {
                    threshold: 10_000_000.0,
                },
            }],
            life: WatchLifeFixture {
                profile: {
                    let mut p = warm_life_profile(false);
                    p.prop_reactions = vec![PropReaction {
                        prop_id: HabitatPropId::new("token_lantern_10m"),
                        intensity: 0.7,
                        kind: PropReactionKind::Glow,
                    }];
                    p
                },
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: Some(fixed_now - Duration::seconds(5)),
            },
            day_context: DayContext {
                day_phase: DayPhase::Dawn,
                wake_resume: Some(WakeResume {
                    from_eval_utc: fixed_now - Duration::hours(3),
                    woke_at_utc: fixed_now - Duration::minutes(2),
                }),
                mature: true,
                ..DayContext::default()
            },
            expected_biome: "Celestial",
            expected_emitter: Some("token_lantern_10m"),
        },
    ]
}

fn species_dialect_props(fixed_now: OffsetDateTime) -> Vec<EarnedHabitatProp> {
    vec![
        EarnedHabitatProp {
            id: HabitatPropId::new("codex_signal_lamp"),
            earned_at: fixed_now - Duration::days(12),
            source: HabitatPropSource::ProviderFirstUse {
                provider_surface: "codex".to_string(),
            },
        },
        EarnedHabitatProp {
            id: HabitatPropId::new("token_shard_1m"),
            earned_at: fixed_now - Duration::days(11),
            source: HabitatPropSource::LifetimeTokens {
                threshold: 1_000_000.0,
            },
        },
        EarnedHabitatProp {
            id: HabitatPropId::new("token_orbit_5m"),
            earned_at: fixed_now - Duration::days(9),
            source: HabitatPropSource::LifetimeTokens {
                threshold: 5_000_000.0,
            },
        },
        EarnedHabitatProp {
            id: HabitatPropId::new("token_lantern_10m"),
            earned_at: fixed_now - Duration::days(6),
            source: HabitatPropSource::LifetimeTokens {
                threshold: 10_000_000.0,
            },
        },
    ]
}

fn species_dialect_frame_fixtures(ctx: &PreviewRenderContext) -> Vec<AliveRoomFrameFixture> {
    let fixed_now = ctx.fixed_now;
    let props = species_dialect_props(fixed_now);
    let day_context = DayContext {
        day_phase: DayPhase::Day,
        mature: true,
        ..DayContext::default()
    };
    let mut hot = hot_life_profile(false);
    hot.work_weather = WorkWeather::OutputSparks;

    let mk = |id: &'static str,
              title: &'static str,
              species: Species,
              color_capability: ColorCapability| {
        AliveRoomFrameFixture {
            id,
            title,
            width: 120,
            height: 32,
            species,
            stage: Stage::S6,
            props: props.clone(),
            life: WatchLifeFixture {
                profile: hot.clone(),
                color_capability,
                last_feed_pulse_at: Some(fixed_now - Duration::seconds(5)),
            },
            day_context,
            expected_biome: "Technical",
            expected_emitter: Some("codex_signal_lamp"),
        }
    };

    vec![
        mk(
            "watch-species-dialect-fuzz",
            "Watch Species Dialect Fuzz",
            Species::Fuzz,
            ColorCapability::Truecolor,
        ),
        mk(
            "watch-species-dialect-blob",
            "Watch Species Dialect Blob",
            Species::Blob,
            ColorCapability::Truecolor,
        ),
        mk(
            "watch-species-dialect-ghost",
            "Watch Species Dialect Ghost",
            Species::Ghost,
            ColorCapability::Truecolor,
        ),
        mk(
            "watch-species-dialect-glitch",
            "Watch Species Dialect Glitch",
            Species::Glitch,
            ColorCapability::Truecolor,
        ),
        mk(
            "watch-species-dialect-crystal",
            "Watch Species Dialect Crystal",
            Species::Crystal,
            ColorCapability::Truecolor,
        ),
        mk(
            "watch-species-dialect-mech",
            "Watch Species Dialect Mech",
            Species::Mech,
            ColorCapability::Truecolor,
        ),
        mk(
            "watch-species-dialect-glitch-flat",
            "Watch Species Dialect Glitch Flat",
            Species::Glitch,
            ColorCapability::Flat,
        ),
        mk(
            "watch-species-dialect-crystal-flat",
            "Watch Species Dialect Crystal Flat",
            Species::Crystal,
            ColorCapability::Flat,
        ),
    ]
}

fn alive_room_pet_state(ctx: &PreviewRenderContext, fixture: &AliveRoomFrameFixture) -> PetState {
    let mut state = liveliness_pet_state(ctx);
    state.pet.generated_species = fixture.species;
    state.stage = fixture.stage;
    state.habitat.earned_props = fixture.props.clone();
    state
}

fn render_alive_room_watch_frame(
    ctx: &PreviewRenderContext,
    scratch_dir: &Path,
    fixture: AliveRoomFrameFixture,
) -> Result<PreviewFrame> {
    let state = alive_room_pet_state(ctx, &fixture);
    let mut frame = render_watch_frame_from_state_with_life(
        ctx,
        scratch_dir,
        WatchFrameFixture {
            id: fixture.id,
            title: fixture.title,
            width: fixture.width,
            height: fixture.height,
            state: &state,
            now: ctx.fixed_now,
        },
        Some(&fixture.life),
        Some(fixture.day_context),
        false,
        seed_usage_store,
    )?;

    let mut vm = crate::tui::view_model::WatchViewModel::fixture();
    vm.pet_render.generated_species = fixture.species;
    vm.pet_render.stage = fixture.stage;
    vm.habitat.earned_props = fixture
        .props
        .iter()
        .map(|prop| {
            let spec = crate::game::habitat::catalog_prop_by_str(prop.id.as_str()).unwrap();
            crate::tui::view_model::EarnedHabitatPropView {
                id: prop.id.clone(),
                earned_at: prop.earned_at,
                kind: spec.kind,
                display_priority: spec.display_priority,
                source: prop.source.clone(),
            }
        })
        .collect();
    vm.life_profile = fixture.life.profile.clone();
    vm.day_context = fixture.day_context;
    vm.last_feed_pulse_at = fixture.life.last_feed_pulse_at;
    let room_profile = crate::tui::room::derive_room_life_profile(&vm, ctx.fixed_now);

    frame.extra_inputs = BTreeMap::from([
        (
            "fixed_now".to_string(),
            Value::String(ctx.fixed_now.unix_timestamp().to_string()),
        ),
        (
            "species".to_string(),
            Value::String(fixture.species.as_str().to_string()),
        ),
        (
            "stage".to_string(),
            Value::String(format!("{:?}", fixture.stage).to_lowercase()),
        ),
        ("terminal_width".to_string(), json!(fixture.width)),
        ("terminal_height".to_string(), json!(fixture.height)),
        (
            "expected_room_life_profile".to_string(),
            json!({
                "primary_biome": fixture.expected_biome,
                "emitter": fixture.expected_emitter,
            }),
        ),
        (
            "room_life_profile".to_string(),
            json!({
                "primary_biome": format!("{:?}", room_profile.biome.primary),
                "secondary_biome": room_profile.biome.secondary.map(|tag| format!("{tag:?}")),
                "emitter": room_profile.resonant_emitter.as_ref().map(|emitter| emitter.prop_id.as_str().to_string()),
                "pet_performance": format!("{:?}", room_profile.pet_performance),
                "scene_moments": room_profile.scene_moments.iter().map(|moment| format!("{:?}", moment.key)).collect::<Vec<_>>(),
            }),
        ),
        (
            "species_dialect".to_string(),
            json!({
                "species": room_profile.species_dialect.species.as_str(),
                "key": room_profile.species_dialect.key.as_str(),
                "status": room_profile.species_dialect.status.as_str()
            }),
        ),
    ]);

    Ok(frame)
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

fn work_life_profile(weather: WorkWeather) -> PetLifeProfile {
    match weather {
        WorkWeather::Clear => idle_life_profile(),
        WorkWeather::CacheMist => cooling_life_profile(),
        _ => {
            let mut profile = warm_life_profile(false);
            profile.work_weather = weather;
            profile
        }
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

fn work_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Day,
        phase_started_at_utc: now - Duration::hours(3),
        phase_ends_at_utc: now + Duration::hours(5),
        today_ratio: 0.6,
        tiredness: 0.1,
        mature: true,
        local_day_started_utc: now - Duration::hours(12),
        local_day_rollover_utc: now + Duration::hours(12),
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

fn dawn_fresh_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Dawn,
        phase_started_at_utc: now - Duration::minutes(40),
        phase_ends_at_utc: now + Duration::minutes(80),
        today_ratio: 0.05,
        tiredness: 0.0,
        mature: true,
        local_day_started_utc: now - Duration::hours(1),
        local_day_rollover_utc: now + Duration::hours(23),
        ..DayContext::default()
    }
}

fn dusk_heavy_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Dusk,
        phase_started_at_utc: now - Duration::hours(1),
        phase_ends_at_utc: now + Duration::hours(2),
        today_ratio: 1.6,
        tiredness: 0.8,
        mature: true,
        local_day_started_utc: now - Duration::hours(12),
        local_day_rollover_utc: now + Duration::hours(12),
        ..DayContext::default()
    }
}

fn night_quiet_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Night,
        phase_started_at_utc: now - Duration::hours(2),
        phase_ends_at_utc: now + Duration::hours(6),
        today_ratio: 0.1,
        tiredness: 0.3,
        mature: true,
        local_day_started_utc: now - Duration::hours(16),
        local_day_rollover_utc: now + Duration::hours(8),
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

fn seed_usage_store_ensemble(path: &Path, now: OffsetDateTime) -> Result<()> {
    let mut usage = UsageStore::open(path)?;
    for (surface, observed_at, effective_tokens, model) in [
        (
            "claude-code",
            now - Duration::minutes(5),
            12_000.0,
            "claude-sonnet",
        ),
        ("codex", now - Duration::minutes(8), 8_000.0, "gpt-5-codex"),
        (
            "gemini",
            now - Duration::minutes(12),
            7_500.0,
            "gemini-2.5-pro",
        ),
        ("opencode", now - Duration::minutes(15), 4_500.0, "unknown"),
    ] {
        usage.insert_event(&NormalizedUsageEvent {
            provider_surface: surface.to_string(),
            model: Some(model.to_string()),
            provider_delta_id: Some(format!("preview-ensemble-{surface}-{effective_tokens}")),
            ..NormalizedUsageEvent::for_test_at(observed_at, effective_tokens)
        })?;
    }
    Ok(())
}

fn seed_usage_store_unknown_source(path: &Path, now: OffsetDateTime) -> Result<()> {
    let mut usage = UsageStore::open(path)?;
    usage.insert_event(&NormalizedUsageEvent {
        provider_surface: "unknown".to_string(),
        model: Some("unrecognized-agent".to_string()),
        provider_delta_id: Some("preview-unknown-6000".to_string()),
        ..NormalizedUsageEvent::for_test_at(now - Duration::minutes(6), 6_000.0)
    })?;
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

        assert_eq!(frames.len(), 47);
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
        assert_eq!(frames[21].id, "watch-daycontext-dawn-fresh");
        assert_eq!((frames[21].width, frames[21].height), (120, 32));
        assert_eq!(frames[22].id, "watch-daycontext-dusk-heavy");
        assert_eq!((frames[22].width, frames[22].height), (120, 32));
        assert_eq!(frames[23].id, "watch-daycontext-night-quiet");
        assert_eq!((frames[23].width, frames[23].height), (120, 32));
        assert_eq!(frames[24].id, "watch-daycontext-work-output-sparks");
        assert_eq!((frames[24].width, frames[24].height), (120, 32));
        assert_eq!(frames[25].id, "watch-daycontext-work-reasoning-pulse");
        assert_eq!((frames[25].width, frames[25].height), (120, 32));
        assert_eq!(frames[26].id, "watch-daycontext-work-cache-mist");
        assert_eq!((frames[26].width, frames[26].height), (120, 32));
        assert_eq!(frames[27].id, "watch-daycontext-work-mixed");
        assert_eq!((frames[27].width, frames[27].height), (120, 32));
        assert_eq!(frames[28].id, "watch-daycontext-work-clear");
        assert_eq!((frames[28].width, frames[28].height), (120, 32));
        assert_eq!(frames[29].id, "room-starter-day-clear");
        assert_eq!((frames[29].width, frames[29].height), (120, 32));
        assert_eq!(frames[30].id, "room-botanical-cache-evening");
        assert_eq!(frames[31].id, "room-technical-output-active");
        assert_eq!(frames[32].id, "room-celestial-artifact-night");
        assert_eq!(frames[33].id, "room-cozy-weekend-quiet");
        assert_eq!(frames[34].id, "room-mixed-full-wide");
        assert_eq!((frames[34].width, frames[34].height), (180, 50));
        assert_eq!(frames[35].id, "room-heavy-day-cozy-large");
        assert_eq!((frames[35].width, frames[35].height), (120, 32));
        assert_eq!(frames[36].id, "room-dawn-wake-small");
        assert_eq!((frames[36].width, frames[36].height), (72, 24));
        assert_eq!(frames[37].id, "watch-species-dialect-fuzz");
        assert_eq!((frames[37].width, frames[37].height), (120, 32));
        assert_eq!(frames[38].id, "watch-species-dialect-blob");
        assert_eq!(frames[39].id, "watch-species-dialect-ghost");
        assert_eq!(frames[40].id, "watch-species-dialect-glitch");
        assert_eq!(frames[41].id, "watch-species-dialect-crystal");
        assert_eq!(frames[42].id, "watch-species-dialect-mech");
        assert_eq!(frames[43].id, "watch-species-dialect-glitch-flat");
        assert_eq!(frames[44].id, "watch-species-dialect-crystal-flat");
        assert_eq!(frames[45].id, "watch-activity-identity-ensemble");
        assert_eq!((frames[45].width, frames[45].height), (120, 32));
        assert_eq!(frames[46].id, "watch-activity-identity-unknown");
        assert_eq!((frames[46].width, frames[46].height), (120, 32));
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
