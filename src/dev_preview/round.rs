use crate::dev_preview::export::{
    PreviewRoundAperture, PreviewRoundMetadata, PreviewRoundPrivacy, PreviewScenarioKind,
};
use crate::dev_preview::frame::PreviewFrame;
use crate::dev_preview::scenarios::{
    color_capability_name, PreviewRenderContext, PreviewScenarioBundle,
};
use crate::round::layout::{RoundAperture, RoundRenderCapabilities, SAFE_INNER_RADIUS_RATIO};
use crate::round::preview::render_round_preview_frame_from_vm;
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

    frames
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
    PreviewScenarioBundle::from_parts(
        frame,
        PreviewScenarioKind::Round,
        "Review round macOS companion preview with aperture masking and privacy metadata.",
        inputs,
        Some(round),
        vec![
            "Confirm the circular aperture masks the frame corners.".to_string(),
            "Check that dashboard labels and source diagnostics are not visible.".to_string(),
            "Verify privacy metadata records all visibility flags as false.".to_string(),
        ],
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
    inputs
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
