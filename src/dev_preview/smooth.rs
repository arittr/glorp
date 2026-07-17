use crate::dev_preview::contract::{
    PreviewLocomotionArtifact, PreviewSmoothParityArtifact, PreviewSmoothPlanArtifact,
};
use crate::dev_preview::export::{
    PreviewDimensions, PreviewPlayback, PreviewRoundAperture, PreviewRoundMetadata,
    PreviewRoundPrivacy, PreviewScenarioKind, PreviewStrip, PreviewStripFrame,
    PreviewStripFrameFiles, PreviewStripKind,
};
use crate::dev_preview::frame::{mark_continuations, PreviewCell, PreviewFrame};
use crate::dev_preview::scenarios::{PreviewRenderContext, PreviewScenarioBundle};
use crate::dev_preview::strips::PreviewStripBundle;
use crate::presentation::smooth::classic_flatten_checksum;
use crate::round::layout::{RoundAperture, SAFE_INNER_RADIUS_RATIO};
use crate::round::locomotion::{
    sample_companion_locomotion, stable_companion_identity, CompanionLocomotionSample,
    LOCOMOTION_SEGMENT_SECS,
};
use crate::round::scene::{build_round_scene_draw_list, CompanionMotion};
use crate::round::smooth::{build_round_smooth_scene_plan, try_build_round_smooth_scene_plan};
use crate::tui::view_model::WatchViewModel;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

const GRID_COLS: u16 = 52;
const GRID_ROWS: u16 = 52;
const DEPTH_GRID_COLS: u16 = 44;
const DEPTH_GRID_ROWS: u16 = 18;
const LOCOMOTION_FRAME_DURATION_MS: u16 = 160;
const REVIEWED_LOCOMOTION_ROUTE_START_UNIX_SECONDS: i64 = 1_760_000_000;

pub const SMOOTH_BASELINE_ID: &str = "round-smooth-classic-baseline";
pub const SMOOTH_PARITY_ID: &str = "round-smooth-classic-parity";
pub const PURPOSEFUL_LOCOMOTION_ID: &str = "round-purposeful-locomotion";
pub const SMOOTH_DEPTH_FAR_ID: &str = "round-smooth-depth-far";
pub const SMOOTH_DEPTH_NEUTRAL_ID: &str = "round-smooth-depth-neutral";
pub const SMOOTH_DEPTH_FRONT_ID: &str = "round-smooth-depth-front";

struct PurposefulLocomotionSample {
    index: usize,
    phase_label: &'static str,
    locomotion: CompanionLocomotionSample,
    plan: crate::presentation::smooth::SmoothCompanionScenePlan,
}

pub fn smooth_bundles(ctx: &PreviewRenderContext) -> Vec<PreviewScenarioBundle> {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let motion = CompanionMotion::default();
    let classic = build_round_scene_draw_list(&vm, ctx.fixed_now, GRID_COLS, GRID_ROWS, &motion);
    let classic_checksum = classic_flatten_checksum(&classic.draw_list.cells);
    let plan = build_round_smooth_scene_plan(&vm, ctx.fixed_now, GRID_COLS, GRID_ROWS, &motion, 0);

    let baseline = scene_draw_list_to_preview_frame(
        SMOOTH_BASELINE_ID,
        "Smooth Classic Baseline",
        GRID_COLS,
        GRID_ROWS,
        &classic.draw_list,
    );
    let mut parity = scene_draw_list_to_preview_frame(
        SMOOTH_PARITY_ID,
        "Smooth Classic Parity",
        GRID_COLS,
        GRID_ROWS,
        &plan.flatten_classic_cells(),
    );
    parity.contract.smooth_plan = Some(PreviewSmoothPlanArtifact::from_scene_plan(
        &parity.id, &vm, &plan,
    ));
    parity.contract.smooth_parity = Some(PreviewSmoothParityArtifact::from_scene_plan(
        &parity.id,
        SMOOTH_BASELINE_ID,
        &vm,
        classic_checksum,
        &plan,
    ));

    let baseline_bundle = PreviewScenarioBundle::from_parts(
        baseline,
        PreviewScenarioKind::Smooth,
        "Review the current Classic cell baseline before comparing Renderer v2 parity.",
        smooth_inputs(ctx, "classic-baseline"),
        Some(smooth_round_metadata(GRID_COLS, GRID_ROWS)),
        vec![
            "Confirm the baseline reads like the current round companion cell fixture.".to_string(),
            "Use this frame as the checksum and visual baseline for smooth parity.".to_string(),
        ],
    );
    let parity_bundle = PreviewScenarioBundle::from_parts(
        parity,
        PreviewScenarioKind::Smooth,
        "Review Renderer v2 parity against the same deterministic Classic fixture.",
        smooth_inputs(ctx, "smooth-parity"),
        Some(smooth_round_metadata(GRID_COLS, GRID_ROWS)),
        vec![
            "Confirm the parity frame still reads as the current Glorp companion.".to_string(),
            "Inspect smooth-plan and smooth-parity sidecars for layer coverage and checksum parity."
                .to_string(),
        ],
    );

    let mut bundles = vec![baseline_bundle, parity_bundle];
    bundles.extend(smooth_depth_bundles(ctx));
    bundles
}

pub fn smooth_depth_bundles(ctx: &PreviewRenderContext) -> Vec<PreviewScenarioBundle> {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let review_motion = CompanionMotion {
        wander_half: 8,
        drift_x_frac: 0.0,
        drift_y_frac: 0.0,
        drift_period_secs: 22,
        upward_bias: 0.0,
        wander: true,
    };

    [
        (SMOOTH_DEPTH_FAR_ID, "Smooth Depth Far", -1.0f32, "far"),
        (
            SMOOTH_DEPTH_NEUTRAL_ID,
            "Smooth Depth Neutral",
            0.0f32,
            "neutral",
        ),
        (SMOOTH_DEPTH_FRONT_ID, "Smooth Depth Front", 1.0f32, "front"),
    ]
    .into_iter()
    .map(|(id, title, depth, plane)| {
        let plan = crate::round::smooth::try_build_round_smooth_scene_plan_with_options(
            &vm,
            ctx.fixed_now,
            DEPTH_GRID_COLS,
            DEPTH_GRID_ROWS,
            &review_motion,
            0,
            crate::round::smooth::SmoothSceneBuildOptions {
                depth_override: Some(depth),
                ..Default::default()
            },
        )
        .expect("depth endpoint preview should build");
        let mut frame = scene_draw_list_to_preview_frame(
            id,
            title,
            DEPTH_GRID_COLS,
            DEPTH_GRID_ROWS,
            &plan.flatten_classic_cells(),
        );
        frame.contract.smooth_plan =
            Some(PreviewSmoothPlanArtifact::from_scene_plan(id, &vm, &plan));

        PreviewScenarioBundle::from_parts(
            frame,
            PreviewScenarioKind::Smooth,
            "Review depth placement using the Smooth typed plan; Classic text/cell captures are intentionally flat.",
            BTreeMap::from([
                (
                    "fixture".to_string(),
                    Value::String("full-tank-depth".to_string()),
                ),
                ("depth".to_string(), json!(depth)),
                ("plane".to_string(), Value::String(plane.to_string())),
                (
                    "cell_capture_geometry".to_string(),
                    Value::String("classic-flat".to_string()),
                ),
                (
                    "depth_geometry_authority".to_string(),
                    Value::String("smooth-plan".to_string()),
                ),
                (
                    "native_hud_overlap_review".to_string(),
                    Value::String("task-7".to_string()),
                ),
            ]),
            Some(smooth_round_metadata(DEPTH_GRID_COLS, DEPTH_GRID_ROWS)),
            vec![
                format!(
                    "The Smooth typed plan is the depth-geometry authority; confirm the pet reads at the {plane} plane without aperture clipping."
                ),
                "Classic text/cell captures are intentionally flat; native HUD overlap is Task 7."
                    .to_string(),
            ],
        )
    })
    .collect()
}

pub fn smooth_strips(_ctx: &PreviewRenderContext) -> Vec<PreviewStripBundle> {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let motion = crate::round::scene::companion_roam_motion();
    let samples = purposeful_locomotion_samples(&vm, &motion);
    let mut frames = Vec::with_capacity(samples.len());
    let mut manifest_frames = Vec::with_capacity(samples.len());

    for sample in samples {
        let mut frame = scene_draw_list_to_preview_frame(
            format!("{PURPOSEFUL_LOCOMOTION_ID}-frame-{:03}", sample.index),
            format!("Purposeful Locomotion {}", sample.phase_label),
            GRID_COLS,
            GRID_ROWS,
            &sample.plan.flatten_classic_cells(),
        );
        frame.contract.locomotion = Some(PreviewLocomotionArtifact::from_sample(
            PURPOSEFUL_LOCOMOTION_ID,
            sample.locomotion,
        ));
        frames.push(frame);
        manifest_frames.push(PreviewStripFrame {
            index: sample.index as u16,
            phase: sample.phase_label.to_string(),
            // The review timeline is deliberately compact and public. It does
            // not reveal the physical route timestamps used to sample the
            // deterministic production locomotion contract.
            elapsed_ms: sample.index as u16 * LOCOMOTION_FRAME_DURATION_MS,
            files: purposeful_locomotion_frame_paths(sample.index),
        });
    }

    vec![PreviewStripBundle {
        manifest: PreviewStrip {
            id: PURPOSEFUL_LOCOMOTION_ID.to_string(),
            kind: PreviewStripKind::PurposefulLocomotion,
            title: "Purposeful Locomotion".to_string(),
            intent: "Review paused, deterministic locomotion choices across waypoint, turn, swim, and depth samples."
                .to_string(),
            dimensions: PreviewDimensions {
                width: GRID_COLS,
                height: GRID_ROWS,
            },
            target_id: "pet-body".to_string(),
            playback: PreviewPlayback {
                starts_paused: true,
                frame_duration_ms: LOCOMOTION_FRAME_DURATION_MS,
            },
            inputs: BTreeMap::from([
                (
                    "fixture".to_string(),
                    Value::String("purposeful-locomotion".to_string()),
                ),
                (
                    "review_facts".to_string(),
                    json!(["waypoint", "turn", "swim", "depth"]),
                ),
            ]),
            frames: manifest_frames,
            review_prompts: vec![
                "Step through waypoint, turn, swim, and depth samples; confirm the strip reads like an animal choosing a destination rather than an oscillator."
                    .to_string(),
                "Confirm facing changes at a waypoint and depth changes only during a later swim."
                    .to_string(),
            ],
        },
        frames,
    }]
}

fn purposeful_locomotion_samples(
    vm: &WatchViewModel,
    motion: &CompanionMotion,
) -> Vec<PurposefulLocomotionSample> {
    let identity = stable_companion_identity(&vm.pet_render.seed);
    let route_start = segment_boundary_at_or_after(reviewed_locomotion_route_start());
    let planar = find_planar_segment(identity, route_start, vm, motion)
        .expect("review fixture should include a planar locomotion segment");
    let turn = planar + time::Duration::seconds(LOCOMOTION_SEGMENT_SECS);
    let depth = find_depth_excursion(
        identity,
        planar + time::Duration::seconds(LOCOMOTION_SEGMENT_SECS),
        vm,
        motion,
    )
    .expect("review fixture should include a later depth excursion");
    let next_boundary = planar + time::Duration::seconds(LOCOMOTION_SEGMENT_SECS);
    let swim_duration = next_boundary - planar;
    let glide_end = next_boundary - time::Duration::milliseconds(1);
    let depth_glide_end = depth + time::Duration::seconds(LOCOMOTION_SEGMENT_SECS);
    let depth_duration = depth_glide_end - depth;
    let depth_midpoint = depth + depth_duration / 2;

    [
        ("waypoint-start", planar),
        ("swim-quarter", planar + swim_duration / 4),
        ("swim-half", planar + swim_duration / 2),
        ("swim-three-quarters", planar + swim_duration * 3 / 4),
        ("swim-end", glide_end),
        ("turn-boundary", turn),
        ("depth-quarter", depth + depth_duration / 4),
        ("depth-excursion", depth_midpoint),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (phase_label, now))| {
        let locomotion = sample_companion_locomotion(identity, now, vm.facing);
        let plan = preview_plan(vm, motion, now)
            .expect("purposeful locomotion preview frame should build");
        PurposefulLocomotionSample { index, phase_label, locomotion, plan }
    })
    .collect()
}

fn reviewed_locomotion_route_start() -> time::OffsetDateTime {
    time::OffsetDateTime::from_unix_timestamp(REVIEWED_LOCOMOTION_ROUTE_START_UNIX_SECONDS)
        .expect("review locomotion route timestamp should parse")
}

fn segment_boundary_at_or_after(now: time::OffsetDateTime) -> time::OffsetDateTime {
    let seconds = now.unix_timestamp();
    let seconds_into_segment = seconds.rem_euclid(LOCOMOTION_SEGMENT_SECS);
    let boundary_seconds = if seconds_into_segment == 0 {
        seconds
    } else {
        seconds + LOCOMOTION_SEGMENT_SECS - seconds_into_segment
    };
    time::OffsetDateTime::from_unix_timestamp(boundary_seconds)
        .expect("review segment boundary should parse")
}

fn find_planar_segment(
    identity: u64,
    start: time::OffsetDateTime,
    vm: &WatchViewModel,
    motion: &CompanionMotion,
) -> Option<time::OffsetDateTime> {
    (0..64)
        .map(|offset| start + time::Duration::seconds(offset * LOCOMOTION_SEGMENT_SECS))
        .find(|segment_start| {
            let glide_end = *segment_start + time::Duration::seconds(LOCOMOTION_SEGMENT_SECS)
                - time::Duration::milliseconds(1);
            let next_boundary = *segment_start + time::Duration::seconds(LOCOMOTION_SEGMENT_SECS);
            let start_sample = sample_companion_locomotion(identity, *segment_start, 1);
            let end_sample = sample_companion_locomotion(identity, glide_end, 1);
            let glide_duration = next_boundary - *segment_start;
            let preview_times = [
                *segment_start,
                *segment_start + glide_duration / 4,
                *segment_start + glide_duration / 2,
                *segment_start + glide_duration * 3 / 4,
                glide_end,
            ];
            (start_sample.point.z - end_sample.point.z).abs() < 0.01
                && sample_companion_locomotion(identity, *segment_start, 1).facing
                    != sample_companion_locomotion(identity, next_boundary, 1).facing
                && preview_times
                    .into_iter()
                    .all(|now| preview_plan(vm, motion, now).is_ok())
                && preview_plan(vm, motion, next_boundary).is_ok()
        })
}

fn find_depth_excursion(
    identity: u64,
    start: time::OffsetDateTime,
    vm: &WatchViewModel,
    motion: &CompanionMotion,
) -> Option<time::OffsetDateTime> {
    (0..64)
        .map(|offset| start + time::Duration::seconds(offset * LOCOMOTION_SEGMENT_SECS))
        .find(|segment_start| {
            let end = *segment_start + time::Duration::seconds(LOCOMOTION_SEGMENT_SECS);
            let midpoint = *segment_start + (end - *segment_start) / 2;
            let start_sample = sample_companion_locomotion(identity, *segment_start, 1);
            let midpoint_sample = sample_companion_locomotion(identity, midpoint, 1);
            (start_sample.point.z - midpoint_sample.point.z).abs() >= 0.10
                && preview_plan(vm, motion, midpoint).is_ok()
        })
}

fn preview_plan(
    vm: &WatchViewModel,
    motion: &CompanionMotion,
    now: time::OffsetDateTime,
) -> Result<
    crate::presentation::smooth::SmoothCompanionScenePlan,
    crate::round::smooth::SmoothScenePlanError,
> {
    try_build_round_smooth_scene_plan(vm, now, GRID_COLS, GRID_ROWS, motion, 0)
}

fn smooth_inputs(ctx: &PreviewRenderContext, mode: &str) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "fixed_now".to_string(),
            Value::String(
                ctx.fixed_now
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            ),
        ),
        (
            "fixture".to_string(),
            Value::String("round-smooth-habitat-props".to_string()),
        ),
        ("renderer".to_string(), Value::String("smooth".to_string())),
        ("mode".to_string(), Value::String(mode.to_string())),
        ("grid_cols".to_string(), json!(GRID_COLS)),
        ("grid_rows".to_string(), json!(GRID_ROWS)),
    ])
}

fn smooth_round_metadata(width: u16, height: u16) -> PreviewRoundMetadata {
    let aperture = RoundAperture::new(width, height);
    PreviewRoundMetadata {
        target_renderer: "smooth-preview-cells",
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

fn purposeful_locomotion_frame_paths(index: usize) -> PreviewStripFrameFiles {
    PreviewStripFrameFiles {
        text: PathBuf::from(format!(
            "strips/{PURPOSEFUL_LOCOMOTION_ID}/frame-{index:03}.txt"
        )),
        cells: PathBuf::from(format!(
            "strips/{PURPOSEFUL_LOCOMOTION_ID}/frame-{index:03}.cells.json"
        )),
        pixel: None,
        locomotion: Some(PathBuf::from(format!(
            "strips/{PURPOSEFUL_LOCOMOTION_ID}/frame-{index:03}.locomotion.json"
        ))),
    }
}

pub(crate) fn scene_draw_list_to_preview_frame(
    id: impl Into<String>,
    title: impl Into<String>,
    width: u16,
    height: u16,
    draw_list: &crate::presentation::SceneDrawList,
) -> PreviewFrame {
    let aperture = RoundAperture::new(width, height);
    let grid = crate::presentation::rasterize(draw_list, width, height);
    let mut cells = Vec::with_capacity(width as usize * height as usize);

    for row in 0..height {
        for col in 0..width {
            let outside_aperture = !aperture.contains(col as f32, row as f32);
            if outside_aperture {
                cells.push(PreviewCell {
                    x: col,
                    y: row,
                    symbol: " ".to_string(),
                    display_width: 1,
                    continuation: false,
                    fg: None,
                    bg: None,
                    modifiers: Vec::new(),
                    outside_aperture: true,
                });
            } else {
                let raster = &grid[row as usize][col as usize];
                let symbol = raster.glyph.to_string();
                let display_width = ratatui::text::Line::from(symbol.clone()).width();
                cells.push(PreviewCell {
                    x: col,
                    y: row,
                    symbol,
                    display_width,
                    continuation: false,
                    fg: raster
                        .fg
                        .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)),
                    bg: raster
                        .bg
                        .map(|c| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)),
                    modifiers: Vec::new(),
                    outside_aperture: false,
                });
            }
        }
    }

    mark_continuations(&mut cells, width);

    PreviewFrame {
        id: id.into(),
        title: title.into(),
        width,
        height,
        cells,
        layout: None,
        extra_inputs: BTreeMap::new(),
        contract: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purposeful_locomotion_fixture_covers_waypoint_turn_swim_and_depth() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let motion = crate::round::scene::companion_roam_motion();
        let samples = purposeful_locomotion_samples(&vm, &motion);
        let labels = samples
            .iter()
            .map(|sample| sample.phase_label)
            .collect::<Vec<_>>();

        assert_eq!(
            labels,
            vec![
                "waypoint-start",
                "swim-quarter",
                "swim-half",
                "swim-three-quarters",
                "swim-end",
                "turn-boundary",
                "depth-quarter",
                "depth-excursion",
            ]
        );
        assert!(matches!(
            samples[0].locomotion.phase,
            crate::round::locomotion::LocomotionPhase::Glide
        ));
        assert!(
            (samples[7].locomotion.point.z - samples[6].locomotion.point.z).abs() > 0.10,
            "later sample should show a depth excursion"
        );
    }
}
