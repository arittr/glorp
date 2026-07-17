use crate::dev_preview::contract::PreviewSmoothPlanArtifact;
use crate::dev_preview::export::{
    PreviewRoundAperture, PreviewRoundMetadata, PreviewRoundPrivacy, PreviewScenarioKind,
};
use crate::dev_preview::frame::PreviewFrame;
use crate::dev_preview::scenarios::{
    color_capability_name, PreviewRenderContext, PreviewScenarioBundle,
};
use crate::round::layout::{RoundAperture, RoundRenderCapabilities, SAFE_INNER_RADIUS_RATIO};
use crate::round::preview::render_round_preview_frame_from_vm;
use crate::round::scene::CompanionMotion;
use crate::round::smooth::{
    try_build_round_smooth_scene_plan_with_options, SmoothSceneBuildOptions,
};
use crate::tui::day::WakeResume;
use crate::tui::identity::SourceDiversity;
use crate::tui::view_model::{EarnedHabitatPropView, SourceStatus, WatchViewModel};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use time::Duration;

pub fn round_frames(ctx: &PreviewRenderContext) -> Vec<PreviewFrame> {
    let mut frames = Vec::new();

    let normal = WatchViewModel::fixture_with_habitat_props();
    frames.push(render_round_preview_frame(
        "round-normal",
        "Round Normal",
        &normal,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let full_cast = retained_composition_full_cast_fixture(ctx.fixed_now.date());
    frames.push(render_round_preview_frame(
        "round-retained-composition-full-cast",
        "Round Retained Composition Full Cast",
        &full_cast,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut missing_yesterday = WatchViewModel::fixture_with_habitat_props();
    set_daily_comparison(
        &mut missing_yesterday,
        842_000_000.0,
        None,
        crate::usage::snapshot::SnapshotState::Missing,
        Some("yesterday-missing"),
    );
    frames.push(render_round_preview_frame(
        "round-hud-missing-yesterday",
        "Round HUD Missing Yesterday",
        &missing_yesterday,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut stale_yesterday = WatchViewModel::fixture_with_habitat_props();
    set_daily_comparison(
        &mut stale_yesterday,
        842_000_000.0,
        Some(900_000_000.0),
        crate::usage::snapshot::SnapshotState::Stale,
        Some("yesterday-stale"),
    );
    frames.push(render_round_preview_frame(
        "round-hud-stale-yesterday",
        "Round HUD Stale Yesterday",
        &stale_yesterday,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut zero_yesterday = WatchViewModel::fixture_with_habitat_props();
    set_daily_comparison(
        &mut zero_yesterday,
        842_000_000.0,
        Some(0.0),
        crate::usage::snapshot::SnapshotState::Current,
        Some("yesterday-zero"),
    );
    frames.push(render_round_preview_frame(
        "round-hud-zero-yesterday",
        "Round HUD Zero Yesterday",
        &zero_yesterday,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut over_yesterday = WatchViewModel::fixture_with_habitat_props();
    set_daily_comparison(
        &mut over_yesterday,
        842_000_000.0,
        Some(678_000_000.0),
        crate::usage::snapshot::SnapshotState::Current,
        None,
    );
    frames.push(render_round_preview_frame(
        "round-hud-over-yesterday",
        "Round HUD Over Yesterday",
        &over_yesterday,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut multi_rollover = WatchViewModel::fixture_with_habitat_props();
    set_daily_comparison(
        &mut multi_rollover,
        1_310_000_000.0,
        Some(500_000_000.0),
        crate::usage::snapshot::SnapshotState::Current,
        None,
    );
    frames.push(render_round_preview_frame(
        "round-hud-multi-rollover",
        "Round HUD Multi Rollover",
        &multi_rollover,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut idle_pace = WatchViewModel::fixture_with_habitat_props();
    idle_pace.rate_momentum.pulse.current_tokens = 0.0;
    frames.push(render_round_preview_frame(
        "round-hud-idle-pace",
        "Round HUD Idle Pace",
        &idle_pace,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut burst_pace = WatchViewModel::fixture_with_habitat_props();
    burst_pace.rate_momentum.pulse.current_tokens = 100_000_000.0;
    frames.push(render_round_preview_frame(
        "round-hud-burst-pace",
        "Round HUD Burst Pace",
        &burst_pace,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut active = WatchViewModel::fixture_with_habitat_props();
    active.activity_identity.source_diversity = SourceDiversity::DualLane;
    active.last_feed_pulse_at = Some(ctx.fixed_now - Duration::milliseconds(400));
    frames.push(render_round_preview_frame(
        "round-active-pulse",
        "Round Active Pulse",
        &active,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut asleep = WatchViewModel::fixture_with_habitat_props();
    asleep.day_context.asleep = true;
    asleep.life_profile.calm_mode = true;
    frames.push(render_round_preview_frame(
        "round-asleep-night",
        "Round Asleep Night",
        &asleep,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut trouble = WatchViewModel::fixture_with_habitat_props();
    trouble.source_health[0].status = SourceStatus::Diagnostic;
    frames.push(render_round_preview_frame(
        "round-helper-trouble",
        "Round Helper Trouble",
        &trouble,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let flat = WatchViewModel::fixture_with_habitat_props();
    frames.push(render_round_preview_frame(
        "round-flat-color",
        "Round Flat Color",
        &flat,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_flat(),
    ));

    let mut glitch = WatchViewModel::fixture_with_habitat_props();
    glitch.pet_render.generated_species = crate::pet::generation::Species::Glitch;
    frames.push(render_round_preview_frame(
        "round-glitch-dialect",
        "Round Glitch Dialect",
        &glitch,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut crystal = WatchViewModel::fixture_with_habitat_props();
    crystal.pet_render.generated_species = crate::pet::generation::Species::Crystal;
    frames.push(render_round_preview_frame(
        "round-crystal-dialect",
        "Round Crystal Dialect",
        &crystal,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut patched_glitch = WatchViewModel::fixture_with_habitat_props();
    patched_glitch.pet_render.seed = "glorp-preview-glitch-persistence".to_string();
    patched_glitch.pet_render.generated_species = crate::pet::generation::Species::Glitch;
    patched_glitch.pet_render.stage = crate::game::evolution::Stage::S6;
    patched_glitch.day_context.date_seed = 42;
    patched_glitch.day_context.today_ratio = 1.7;
    patched_glitch.life_profile.burst_level = 0.0;
    patched_glitch.life_profile.calm_mode = true;
    crate::commands::watch::rerender_pet_for_view_model(
        &mut patched_glitch,
        ctx.fixed_now.unix_timestamp().max(0) as u64,
        false,
        ctx.fixed_now,
    )
    .expect("round preview fixture should rerender");
    frames.push(render_round_preview_frame(
        "round-glitch-patched-s6",
        "Round Glitch Patched S6",
        &patched_glitch,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    frames.extend(spatial_cue_frames(ctx));

    frames
}

fn spatial_cue_frames(ctx: &PreviewRenderContext) -> Vec<PreviewFrame> {
    let mut frames = Vec::new();
    for (id, title, mood) in [
        (
            "round-spatial-rim-content-idle",
            "Round Spatial Rim Content Idle",
            crate::game::metabolism::Mood::Content,
        ),
        (
            "round-spatial-rim-sad-idle",
            "Round Spatial Rim Sad Idle",
            crate::game::metabolism::Mood::Sad,
        ),
        (
            "round-spatial-rim-sleepy-idle",
            "Round Spatial Rim Sleepy Idle",
            crate::game::metabolism::Mood::Sleepy,
        ),
    ] {
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        vm.pet_render.mood = mood;
        frames.push(render_round_preview_frame(
            id,
            title,
            &vm,
            ctx.fixed_now,
            52,
            52,
            RoundRenderCapabilities::preview_truecolor(),
        ));
    }

    let mut active = WatchViewModel::fixture_with_habitat_props();
    active.last_feed_pulse_at = Some(ctx.fixed_now - Duration::milliseconds(400));
    frames.push(render_round_preview_frame(
        "round-spatial-rim-active",
        "Round Spatial Rim Active",
        &active,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let disabled = WatchViewModel::fixture_with_habitat_props();
    frames.push(render_round_preview_frame(
        "round-spatial-rim-disabled",
        "Round Spatial Rim Disabled",
        &disabled,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    for (id, title) in [
        (
            "round-spatial-stats-behind",
            "Round Spatial Statistics Behind",
        ),
        (
            "round-spatial-stats-interacting",
            "Round Spatial Statistics Interacting",
        ),
        (
            "round-spatial-stats-front",
            "Round Spatial Statistics Front",
        ),
    ] {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let depth = spatial_cue_statistics_depth(id)
            .expect("statistics fixture must carry an explicit review depth");
        frames.push(render_statistics_depth_review_frame(
            id,
            title,
            &vm,
            ctx.fixed_now,
            depth,
        ));
    }

    let settle = Duration::seconds(crate::pet::animator::WANDER_SETTLE_SECS);
    let mut asleep = WatchViewModel::fixture_with_habitat_props();
    asleep.day_context.asleep = true;
    asleep.life_profile.calm_mode = true;
    asleep.day_context.sleep_onset_utc = Some(ctx.fixed_now - settle);
    asleep.pet_render.mood = crate::game::metabolism::Mood::Sleepy;
    frames.push(render_round_preview_frame(
        "round-spatial-sleep-settled",
        "Round Spatial Sleep Settled",
        &asleep,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut waking = WatchViewModel::fixture_with_habitat_props();
    waking.day_context.wake_resume = Some(WakeResume {
        from_eval_utc: ctx.fixed_now - settle * 2,
        woke_at_utc: ctx.fixed_now - settle,
    });
    frames.push(render_round_preview_frame(
        "round-spatial-wake-resume",
        "Round Spatial Wake Resume",
        &waking,
        ctx.fixed_now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    // Spatial review only needs the companion HUD's fixed geometry. Replace
    // fixture-derived metrics with static redacted values before export.
    for frame in &mut frames {
        let aperture = RoundAperture::new(frame.width, frame.height);
        frame.contract.hud = Some(
            crate::dev_preview::contract::PreviewHudArtifact::redacted_spatial_cue(
                &frame.id, aperture,
            ),
        );
    }

    frames
}

/// Native statistics projection pixels are only visible in the retained
/// renderer. Preview Lab routes these review-only fixtures through its
/// established smooth-plan seam, then applies that plan to local cells as
/// typed depth evidence. The cells do not render the native HUD projection.
fn render_statistics_depth_review_frame(
    id: &str,
    title: &str,
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    depth: f32,
) -> PreviewFrame {
    let review_motion = CompanionMotion::default();
    let plan = try_build_round_smooth_scene_plan_with_options(
        vm,
        now,
        52,
        52,
        &review_motion,
        0,
        SmoothSceneBuildOptions {
            depth_override: Some(depth),
            // The square Preview Lab capture has square logical cells; its
            // physical aperture must not inherit the grid-only 2:1 fallback.
            viewport_points: Some([52.0, 52.0]),
        },
    )
    .expect("statistics depth preview should build");
    let transformed_cells = crate::presentation::smooth::LayeredPetScene {
        layers: plan.layers.clone(),
        prop_shadow_sources: Vec::new(),
    }
    .flatten_classic_cells();
    let mut frame = render_round_preview_frame(
        id,
        title,
        vm,
        now,
        52,
        52,
        RoundRenderCapabilities::preview_truecolor(),
    );
    frame.cells = crate::dev_preview::smooth::scene_draw_list_to_preview_frame(
        id,
        title,
        52,
        52,
        &transformed_cells,
    )
    .cells;
    frame.contract.smooth_plan = Some(PreviewSmoothPlanArtifact::from_scene_plan(id, vm, &plan));
    frame
}

fn retained_composition_full_cast_fixture(local_date: time::Date) -> WatchViewModel {
    let mut vm = WatchViewModel::fixture_with_tank_inhabitants_for_age(120, local_date);
    vm.habitat.earned_props = crate::game::habitat::HABITAT_PROP_CATALOG
        .iter()
        .map(|spec| EarnedHabitatPropView {
            id: crate::storage::state::HabitatPropId::new(spec.id),
            earned_at: time::OffsetDateTime::UNIX_EPOCH,
            kind: spec.kind,
            display_priority: spec.display_priority,
            source: crate::storage::state::HabitatPropSource::LifetimeTokens {
                threshold: spec.lifetime_threshold.unwrap_or(0.0),
            },
        })
        .collect();
    vm
}

pub fn round_bundles(ctx: &PreviewRenderContext) -> Vec<PreviewScenarioBundle> {
    round_frames(ctx)
        .into_iter()
        .map(|frame| round_bundle(frame, ctx))
        .collect()
}

fn round_bundle(frame: PreviewFrame, ctx: &PreviewRenderContext) -> PreviewScenarioBundle {
    let round = round_metadata(&frame);
    let inputs = round_inputs_for_frame(&frame, ctx);
    let mut review_prompts = vec![
        "Confirm the circular aperture masks the frame corners.".to_string(),
        "Check that dashboard labels and source diagnostics are not visible.".to_string(),
        "Verify privacy metadata records all visibility flags as false.".to_string(),
    ];
    if is_spatial_cue_frame(&frame.id) {
        review_prompts.extend(spatial_cue_review_prompts(&frame.id));
    }
    PreviewScenarioBundle::from_parts(
        frame,
        PreviewScenarioKind::Round,
        "Review round macOS companion preview with aperture masking and privacy metadata.",
        inputs,
        Some(round),
        review_prompts,
    )
}

fn round_inputs(ctx: &PreviewRenderContext) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "color_capability".to_string(),
            Value::String(color_capability_name(ctx.render.color_capability).to_string()),
        ),
        (
            "fixture".to_string(),
            Value::String("seeded-pet-state-and-usage-sqlite".to_string()),
        ),
        (
            "privacy_source_names_visible".to_string(),
            Value::Bool(false),
        ),
        (
            "privacy_exact_counts_visible".to_string(),
            Value::Bool(false),
        ),
        (
            "privacy_diagnostic_text_visible".to_string(),
            Value::Bool(false),
        ),
    ])
}

/// Manifest inputs for the Glitch S6 patched round fixture, derived from the
/// same pet identity, stage, and day-seed the fixture renders with (not the
/// built view-model), so the contract is truthful. Mirrors
/// `watch::glitch_persistence_extra_inputs`.
fn round_inputs_for_frame(
    frame: &PreviewFrame,
    ctx: &PreviewRenderContext,
) -> BTreeMap<String, Value> {
    let mut inputs = round_inputs(ctx);
    if frame.id == "round-glitch-patched-s6" {
        let pet = crate::pet::generation::generate_pet("glorp-preview-glitch-persistence")
            .with_species(crate::pet::generation::Species::Glitch);
        let (raw_lines, raw_spans) =
            crate::dev_preview::pets::raw_glitch_render_for_patch_selection(
                &pet,
                crate::game::evolution::Stage::S6,
            );
        let selected_patch_cells = crate::pet::render::selected_glitch_patch_cells(
            &pet,
            crate::game::evolution::Stage::S6,
            42,
            crate::pet::render::GlitchPatchTier::Heavy,
            &raw_lines,
            &raw_spans,
        );
        let selected_patch_cells_json = selected_patch_cells
            .iter()
            .map(|cell| json!({"row": cell.row, "col": cell.col}))
            .collect::<Vec<_>>();

        inputs.extend([
            ("species".to_string(), json!("glitch")),
            ("stage".to_string(), json!("s6")),
            ("date_seed".to_string(), json!(42_u64)),
            ("patch_tier".to_string(), json!("heavy")),
            ("burst_level".to_string(), json!("none")),
            ("calm_mode".to_string(), json!(true)),
            ("feed_reaction".to_string(), json!(false)),
            (
                "expected_patch_count".to_string(),
                json!(selected_patch_cells.len()),
            ),
            (
                "selected_patch_cells".to_string(),
                json!(selected_patch_cells_json),
            ),
            (
                "protected_face_cells".to_string(),
                crate::dev_preview::frame::protected_face_cells_json(&raw_spans),
            ),
        ]);
    }
    if frame.id == "round-retained-composition-full-cast" {
        inputs.extend([
            (
                "fixture".to_string(),
                json!("retained-composition-full-cast"),
            ),
            (
                "earned_prop_count".to_string(),
                json!(crate::game::habitat::HABITAT_PROP_CATALOG.len()),
            ),
            (
                "earned_inhabitant_count".to_string(),
                json!(crate::game::habitat::TANK_INHABITANT_CATALOG.len()),
            ),
            ("tank_calendar_age_days".to_string(), json!(120)),
            (
                "visible_prop_capacity".to_string(),
                json!(crate::presentation::companion_scene::MAX_VISIBLE_PROPS),
            ),
            (
                "visible_tank_capacity".to_string(),
                json!(crate::presentation::companion_scene::MAX_VISIBLE_TANK_INHABITANTS),
            ),
        ]);
    }
    if let Some(cues) = spatial_cue_inputs(&frame.id) {
        inputs.insert("spatial_cues".to_string(), cues);
    }
    inputs
}

#[derive(Debug, Clone, Copy)]
enum PreviewRimPresentation {
    Production,
    DisabledForReview,
}

impl PreviewRimPresentation {
    fn presentation_options(
        self,
    ) -> crate::presentation::companion_scene::input::CompanionPresentationOptions {
        let standard =
            crate::presentation::companion_scene::input::CompanionPresentationOptions::STANDARD;
        match self {
            Self::Production => standard,
            Self::DisabledForReview => standard.without_pet_rim_for_preview(),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::DisabledForReview => "disabled-for-preview-review",
        }
    }
}

fn is_spatial_cue_frame(id: &str) -> bool {
    id.starts_with("round-spatial-")
}

fn spatial_cue_statistics_depth(id: &str) -> Option<f32> {
    match id {
        "round-spatial-stats-behind" => Some(-0.5),
        "round-spatial-stats-interacting" => Some(0.68),
        "round-spatial-stats-front" => Some(0.9),
        _ => None,
    }
}

fn spatial_cue_inputs(id: &str) -> Option<Value> {
    let (activity, rim_presentation, statistics_depth, locomotion_lifecycle) = match id {
        "round-spatial-rim-content-idle" => {
            (0.0, PreviewRimPresentation::Production, -0.5, "awake")
        }
        "round-spatial-rim-sad-idle" => (0.0, PreviewRimPresentation::Production, -0.5, "awake"),
        "round-spatial-rim-sleepy-idle" => (0.0, PreviewRimPresentation::Production, -0.5, "awake"),
        "round-spatial-rim-active" => (1.0, PreviewRimPresentation::Production, -0.5, "awake"),
        "round-spatial-rim-disabled" => (
            0.0,
            PreviewRimPresentation::DisabledForReview,
            -0.5,
            "awake",
        ),
        "round-spatial-stats-behind"
        | "round-spatial-stats-interacting"
        | "round-spatial-stats-front" => (
            0.0,
            PreviewRimPresentation::Production,
            spatial_cue_statistics_depth(id)
                .expect("statistics cue inputs must use the rendered review depth"),
            "awake",
        ),
        "round-spatial-sleep-settled" => (
            0.0,
            PreviewRimPresentation::Production,
            0.0,
            "sleep-settled",
        ),
        "round-spatial-wake-resume" => {
            (0.0, PreviewRimPresentation::Production, 0.0, "wake-resume")
        }
        _ => return None,
    };
    let presentation_options = rim_presentation.presentation_options();
    let production_rim = crate::presentation::companion_effects::pet_rim_style_with_presentation(
        activity,
        false,
        presentation_options.pet_rim_enabled,
    );
    let rim_enabled = production_rim.enabled;
    let rim_intensity = if !rim_enabled {
        "none"
    } else if activity > 0.0 {
        "active"
    } else {
        "idle"
    };
    let composition = crate::round::depth::CompanionDepthComposition::resolve(statistics_depth)
        .expect("spatial cue fixture depth should be valid");
    let statistics_relation = if composition.statistics_interaction.reveal_mix > 0.0
        && matches!(
            composition.pet_statistics_order,
            crate::round::depth::PetStatisticsOrder::BehindStatistics
        ) {
        "interacting"
    } else if matches!(
        composition.pet_statistics_order,
        crate::round::depth::PetStatisticsOrder::InFrontOfStatistics
    ) {
        "front"
    } else {
        "behind"
    };
    let cell_grid_evidence = if spatial_cue_statistics_depth(id).is_some() {
        "typed-smooth-plan-with-transformed-cells"
    } else {
        "round-cell-grid"
    };

    Some(json!({
        "aura": "absent",
        "evidence": {
            "cell_grid": cell_grid_evidence,
            "hud_projection": "typed-contract-redacted-hud-sidecar-no-native-pixels",
            "pet_rim": "typed-contract-presentation-option-no-native-pixels",
            "native_visual_verification": "native-retained-appkit-tests-or-local-companion-qa",
        },
        "locomotion_lifecycle": locomotion_lifecycle,
        "prop_shadow_order": "front-of-statistics",
        "rim_enabled": rim_enabled,
        "rim_extent": "body-local",
        "rim_intensity": rim_intensity,
        "rim_presentation": rim_presentation.label(),
        "rim_style": {
            "enabled": production_rim.enabled,
            "renderer_input": "presentation-options",
        },
        "statistics_projection": "rear-receiving-surface",
        "statistics_relation": statistics_relation,
    }))
}

fn spatial_cue_review_prompts(id: &str) -> Vec<String> {
    match id {
        "round-spatial-stats-behind"
        | "round-spatial-stats-interacting"
        | "round-spatial-stats-front" => vec![
            "Use the typed Smooth plan and redacted HUD sidecar as statistics-projection contract evidence; Preview Lab cells do not render native HUD projection pixels.".to_string(),
            "Use native retained/AppKit test evidence or final local companion visual QA to judge the native statistics-projection cue.".to_string(),
        ],
        "round-spatial-rim-content-idle"
        | "round-spatial-rim-sad-idle"
        | "round-spatial-rim-sleepy-idle"
        | "round-spatial-rim-active"
        | "round-spatial-rim-disabled" => vec![
            "Preview Lab records the rim as a typed presentation-option contract; its cell grid does not render native rim pixels.".to_string(),
            "The rim-disabled fixture proves only the private preview presentation option. Use native retained/AppKit test evidence or final local companion visual QA to judge rim extent, intensity, and the absence of a broad aura.".to_string(),
        ],
        "round-spatial-sleep-settled" | "round-spatial-wake-resume" => vec![
            "Inspect the actual cell capture for a settled sleep pose or a resumed wake pose; use the purposeful locomotion strip for visual continuity.".to_string(),
        ],
        _ => Vec::new(),
    }
}

fn round_metadata(frame: &PreviewFrame) -> PreviewRoundMetadata {
    let aperture = RoundAperture::new(frame.width, frame.height);
    PreviewRoundMetadata {
        target_renderer: "preview-cells",
        aperture: PreviewRoundAperture {
            shape: "circle",
            center_x: aperture.center_x,
            center_y: aperture.center_y,
            radius: aperture.radius,
            safe_inner_radius: aperture.radius * SAFE_INNER_RADIUS_RATIO,
            transparent_outside_aperture: true,
        },
        privacy: PreviewRoundPrivacy {
            source_names_visible: false,
            exact_counts_visible: false,
            diagnostic_text_visible: false,
        },
    }
}

pub(crate) fn render_round_preview_frame(
    id: &str,
    title: &str,
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
    width: u16,
    height: u16,
    capabilities: RoundRenderCapabilities,
) -> PreviewFrame {
    let mut frame =
        render_round_preview_frame_from_vm(id, title, vm, now, width, height, capabilities);
    let aperture = RoundAperture::new(frame.width, frame.height);
    frame.contract.hud = Some(
        crate::dev_preview::contract::PreviewHudArtifact::from_companion_view_model(
            &frame.id, vm, aperture,
        ),
    );
    frame
}

fn set_daily_comparison(
    vm: &mut WatchViewModel,
    today_tokens: f64,
    yesterday_tokens: Option<f64>,
    yesterday_state: crate::usage::snapshot::SnapshotState,
    reason: Option<&str>,
) {
    vm.today_effective_tokens = today_tokens;
    vm.daily_comparison = crate::tui::view_model::DailyComparison {
        today_provider_day: time::macros::date!(2026 - 07 - 06),
        yesterday_provider_day: time::macros::date!(2026 - 07 - 05),
        today_tokens,
        yesterday_tokens,
        today_snapshot_state: crate::usage::snapshot::SnapshotState::Current,
        yesterday_snapshot_state: yesterday_state,
        today_observed_at: Some(time::macros::datetime!(2026 - 07 - 06 20:00 UTC)),
        yesterday_observed_at: Some(time::macros::datetime!(2026 - 07 - 05 20:00 UTC)),
        unavailable_reason: reason.map(str::to_string),
        fraction_of_yesterday: match (yesterday_tokens, reason) {
            (Some(yesterday), None) if yesterday > 0.0 => Some(today_tokens / yesterday),
            _ => None,
        },
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev_preview::export::PreviewScenarioKind;
    use std::path::PathBuf;

    #[test]
    fn round_bundles_include_round_manifest_metadata() {
        let ctx = PreviewRenderContext::deterministic();

        let bundles = round_bundles(&ctx);

        let normal = bundles
            .iter()
            .find(|bundle| bundle.frame.id == "round-normal")
            .unwrap();
        assert_eq!(normal.scenario.kind, PreviewScenarioKind::Round);
        assert_eq!(
            normal.scenario.files.text,
            PathBuf::from("frames/round-normal.txt")
        );
        let round = normal.scenario.round.as_ref().unwrap();
        assert_eq!(round.target_renderer, "preview-cells");
        assert_eq!(round.aperture.shape, "circle");
        assert!(!round.privacy.source_names_visible);
        assert!(!round.privacy.exact_counts_visible);
        assert!(!round.privacy.diagnostic_text_visible);
    }

    #[test]
    fn round_bundles_include_retained_composition_full_cast_fixture() {
        let ctx = PreviewRenderContext::deterministic();

        let bundles = round_bundles(&ctx);

        let full_cast = bundles
            .iter()
            .find(|bundle| bundle.frame.id == "round-retained-composition-full-cast")
            .expect("retained composition full-cast fixture");
        assert_eq!(
            full_cast.scenario.inputs["earned_prop_count"],
            serde_json::json!(crate::game::habitat::HABITAT_PROP_CATALOG.len())
        );
        assert_eq!(
            full_cast.scenario.inputs["earned_inhabitant_count"],
            serde_json::json!(crate::game::habitat::TANK_INHABITANT_CATALOG.len())
        );
        assert_eq!(
            full_cast.scenario.inputs["tank_calendar_age_days"],
            serde_json::json!(120)
        );
        assert!(full_cast
            .frame
            .cells
            .iter()
            .any(|cell| !cell.outside_aperture && !cell.symbol.trim().is_empty()));
    }
}
