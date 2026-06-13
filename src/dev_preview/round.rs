use crate::dev_preview::frame::PreviewFrame;
use crate::dev_preview::scenarios::PreviewRenderContext;
use crate::round::layout::RoundRenderCapabilities;
use crate::round::preview::render_round_preview_frame_from_vm;
use crate::tui::identity::SourceDiversity;
use crate::tui::view_model::{SourceStatus, WatchViewModel};
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

fn frame(
    id: &str,
    title: &str,
    vm: &WatchViewModel,
    ctx: &PreviewRenderContext,
    capabilities: RoundRenderCapabilities,
) -> PreviewFrame {
    render_round_preview_frame_from_vm(id, title, vm, ctx.fixed_now, 52, 52, capabilities)
}
