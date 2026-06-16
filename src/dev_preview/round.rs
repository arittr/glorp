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
use crate::tui::view_model::{SourceStatus, WatchViewModel};
use serde_json::Value;
use std::collections::BTreeMap;
use time::Duration;

pub fn round_frames(ctx: &PreviewRenderContext) -> Vec<PreviewFrame> {
    let mut frames = Vec::new();

    let normal = WatchViewModel::fixture_with_habitat_props();
    frames.push(frame(
        "round-normal",
        "Round Normal",
        &normal,
        ctx,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut active = WatchViewModel::fixture_with_habitat_props();
    active.activity_identity.source_diversity = SourceDiversity::DualLane;
    active.last_feed_pulse_at = Some(ctx.fixed_now - Duration::milliseconds(400));
    frames.push(frame(
        "round-active-pulse",
        "Round Active Pulse",
        &active,
        ctx,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut asleep = WatchViewModel::fixture_with_habitat_props();
    asleep.day_context.asleep = true;
    asleep.life_profile.calm_mode = true;
    frames.push(frame(
        "round-asleep-night",
        "Round Asleep Night",
        &asleep,
        ctx,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut trouble = WatchViewModel::fixture_with_habitat_props();
    trouble.source_health[0].status = SourceStatus::Diagnostic;
    frames.push(frame(
        "round-helper-trouble",
        "Round Helper Trouble",
        &trouble,
        ctx,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let flat = WatchViewModel::fixture_with_habitat_props();
    frames.push(frame(
        "round-flat-color",
        "Round Flat Color",
        &flat,
        ctx,
        RoundRenderCapabilities::preview_flat(),
    ));

    let mut glitch = WatchViewModel::fixture_with_habitat_props();
    glitch.pet_render.generated_species = crate::pet::generation::Species::Glitch;
    frames.push(frame(
        "round-glitch-dialect",
        "Round Glitch Dialect",
        &glitch,
        ctx,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut crystal = WatchViewModel::fixture_with_habitat_props();
    crystal.pet_render.generated_species = crate::pet::generation::Species::Crystal;
    frames.push(frame(
        "round-crystal-dialect",
        "Round Crystal Dialect",
        &crystal,
        ctx,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    frames
}

pub fn round_bundles(ctx: &PreviewRenderContext) -> Vec<PreviewScenarioBundle> {
    round_frames(ctx)
        .into_iter()
        .map(|frame| round_bundle(frame, ctx))
        .collect()
}

fn round_bundle(frame: PreviewFrame, ctx: &PreviewRenderContext) -> PreviewScenarioBundle {
    let round = round_metadata(&frame);
    PreviewScenarioBundle::from_parts(
        frame,
        PreviewScenarioKind::Round,
        "Review round macOS companion preview with aperture masking and privacy metadata.",
        round_inputs(ctx),
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

fn frame(
    id: &str,
    title: &str,
    vm: &WatchViewModel,
    ctx: &PreviewRenderContext,
    capabilities: RoundRenderCapabilities,
) -> PreviewFrame {
    render_round_preview_frame_from_vm(id, title, vm, ctx.fixed_now, 52, 52, capabilities)
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
}
