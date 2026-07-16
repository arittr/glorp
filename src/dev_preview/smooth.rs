use crate::dev_preview::contract::{
    PreviewSmoothMotionArtifact, PreviewSmoothParityArtifact, PreviewSmoothPlanArtifact,
};
use crate::dev_preview::export::{
    PreviewDimensions, PreviewPlayback, PreviewRoundAperture, PreviewRoundMetadata,
    PreviewRoundPrivacy, PreviewScenarioKind, PreviewStrip, PreviewStripFrame,
    PreviewStripFrameFiles, PreviewStripKind,
};
use crate::dev_preview::frame::{mark_continuations, PreviewCell, PreviewFrame};
use crate::dev_preview::scenarios::{PreviewRenderContext, PreviewScenarioBundle};
use crate::dev_preview::strips::PreviewStripBundle;
use crate::presentation::smooth::{
    classic_flatten_checksum, SmoothLayerMotionBinding, SmoothParallaxPlaneTranslations,
    SmoothPoint,
};
use crate::round::layout::{RoundAperture, SAFE_INNER_RADIUS_RATIO};
use crate::round::scene::{build_round_scene_draw_list, CompanionMotion};
use crate::round::smooth::build_round_smooth_scene_plan;
use crate::tui::view_model::WatchViewModel;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const GRID_COLS: u16 = 52;
const GRID_ROWS: u16 = 52;
const DEPTH_GRID_COLS: u16 = 44;
const DEPTH_GRID_ROWS: u16 = 18;
const MOTION_FRAME_DURATION_MS: u64 = 160;
const MOTION_FRAME_COUNT: usize = 12;
const WANDER_PERIOD_MS: u64 = 22_000;
const REVIEWED_MOTION_START_UNIX_MS: i128 = 1_760_000_001_000;

pub const SMOOTH_BASELINE_ID: &str = "round-smooth-classic-baseline";
pub const SMOOTH_PARITY_ID: &str = "round-smooth-classic-parity";
pub const SMOOTH_MOTION_ID: &str = "round-smooth-motion";
pub const SMOOTH_DEPTH_FAR_ID: &str = "round-smooth-depth-far";
pub const SMOOTH_DEPTH_NEUTRAL_ID: &str = "round-smooth-depth-neutral";
pub const SMOOTH_DEPTH_FRONT_ID: &str = "round-smooth-depth-front";

struct SmoothMotionSample {
    index: usize,
    elapsed_ms: u64,
    now: time::OffsetDateTime,
    semantic_art_tick_index: u64,
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
            "Review the pet's depth-driven vertical placement in the physical tank.",
            BTreeMap::from([
                (
                    "fixture".to_string(),
                    Value::String("full-tank-depth".to_string()),
                ),
                ("depth".to_string(), json!(depth)),
                ("plane".to_string(), Value::String(plane.to_string())),
            ]),
            Some(smooth_round_metadata(DEPTH_GRID_COLS, DEPTH_GRID_ROWS)),
            vec![
                format!("Confirm the pet reads at the {plane} plane without aperture clipping."),
                "Confirm shallow scale remains restrained while vertical placement carries depth."
                    .to_string(),
            ],
        )
    })
    .collect()
}

pub fn smooth_strips(ctx: &PreviewRenderContext) -> Vec<PreviewStripBundle> {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let motion = crate::round::scene::companion_roam_motion();
    let motion_start_now = smooth_motion_start_now(ctx.fixed_now, &vm, &motion);
    let samples = smooth_motion_samples(motion_start_now, &vm, &motion);
    let max_adjacent_parallax_delta_by_plane = max_adjacent_parallax_delta(&samples);
    let mut frames = Vec::with_capacity(MOTION_FRAME_COUNT);
    let mut manifest_frames = Vec::with_capacity(MOTION_FRAME_COUNT);

    for sample in samples {
        let mut frame = scene_draw_list_to_preview_frame(
            format!("{SMOOTH_MOTION_ID}-frame-{:03}", sample.index),
            format!("Smooth Motion Frame {:03}", sample.index),
            GRID_COLS,
            GRID_ROWS,
            &sample.plan.flatten_classic_cells(),
        );
        frame.contract.smooth_motion = Some(PreviewSmoothMotionArtifact::from_scene_plan(
            SMOOTH_MOTION_ID,
            sample.index as u16,
            sample.elapsed_ms,
            sample.now,
            sample.semantic_art_tick_index,
            &vm,
            &sample.plan,
            max_adjacent_parallax_delta_by_plane,
        ));
        frames.push(frame);
        manifest_frames.push(PreviewStripFrame {
            index: sample.index as u16,
            phase: format!("motion-{:03}", sample.index),
            elapsed_ms: sample.elapsed_ms as u16,
            files: smooth_strip_frame_paths(sample.index),
        });
    }

    vec![PreviewStripBundle {
        manifest: PreviewStrip {
            id: SMOOTH_MOTION_ID.to_string(),
            kind: PreviewStripKind::SmoothMotion,
            title: "Smooth Motion".to_string(),
            intent: "Review deterministic smooth pet bob metadata across a parity strip."
                .to_string(),
            dimensions: PreviewDimensions {
                width: GRID_COLS,
                height: GRID_ROWS,
            },
            target_id: "pet-body".to_string(),
            playback: PreviewPlayback {
                starts_paused: true,
                frame_duration_ms: MOTION_FRAME_DURATION_MS as u16,
            },
            inputs: BTreeMap::from([
                (
                    "fixture".to_string(),
                    Value::String("round-smooth-motion".to_string()),
                ),
                (
                    "frame_duration_ms".to_string(),
                    json!(MOTION_FRAME_DURATION_MS),
                ),
                ("frame_count".to_string(), json!(MOTION_FRAME_COUNT)),
                ("now_advances_with_elapsed".to_string(), json!(true)),
                (
                    "motion_start_unix_ms".to_string(),
                    json!(
                        i128::from(motion_start_now.unix_timestamp()) * 1_000
                            + i128::from(motion_start_now.millisecond())
                    ),
                ),
            ]),
            frames: manifest_frames,
            review_prompts: vec![
                "Step through the strip and compare bob metadata even when the cell grid barely changes."
                    .to_string(),
                "Confirm pet-body motion stays fractional and deterministic across frames."
                    .to_string(),
            ],
        },
        frames,
    }]
}

fn smooth_motion_start_now(
    fixed_now: time::OffsetDateTime,
    vm: &WatchViewModel,
    motion: &CompanionMotion,
) -> time::OffsetDateTime {
    let reviewed_start = reviewed_motion_start_now();
    if smooth_motion_window_satisfies_preview_contract(reviewed_start, vm, motion) {
        return reviewed_start;
    }

    for offset_ms in
        (MOTION_FRAME_DURATION_MS..WANDER_PERIOD_MS).step_by(MOTION_FRAME_DURATION_MS as usize)
    {
        let candidate = reviewed_start + time::Duration::milliseconds(offset_ms as i64);
        if smooth_motion_window_satisfies_preview_contract(candidate, vm, motion) {
            return candidate;
        }
    }

    fixed_now
}

fn reviewed_motion_start_now() -> time::OffsetDateTime {
    time::OffsetDateTime::from_unix_timestamp_nanos(REVIEWED_MOTION_START_UNIX_MS * 1_000_000)
        .expect("reviewed motion start timestamp should parse")
}

fn smooth_motion_samples(
    start_now: time::OffsetDateTime,
    vm: &WatchViewModel,
    motion: &CompanionMotion,
) -> Vec<SmoothMotionSample> {
    (0..MOTION_FRAME_COUNT)
        .map(|index| {
            let elapsed_ms = index as u64 * MOTION_FRAME_DURATION_MS;
            let now = start_now + time::Duration::milliseconds(elapsed_ms as i64);
            let plan =
                build_round_smooth_scene_plan(vm, now, GRID_COLS, GRID_ROWS, motion, elapsed_ms);

            SmoothMotionSample {
                index,
                elapsed_ms,
                now,
                semantic_art_tick_index: elapsed_ms / 250,
                plan,
            }
        })
        .collect()
}

fn point_delta(previous: SmoothPoint, current: SmoothPoint) -> SmoothPoint {
    SmoothPoint {
        x: (current.x - previous.x).abs(),
        y: (current.y - previous.y).abs(),
    }
}

fn max_point(left: SmoothPoint, right: SmoothPoint) -> SmoothPoint {
    SmoothPoint {
        x: left.x.max(right.x),
        y: left.y.max(right.y),
    }
}

fn plane_delta(
    previous: SmoothParallaxPlaneTranslations,
    current: SmoothParallaxPlaneTranslations,
) -> SmoothParallaxPlaneTranslations {
    SmoothParallaxPlaneTranslations {
        far: point_delta(previous.far, current.far),
        mid: point_delta(previous.mid, current.mid),
        behind: point_delta(previous.behind, current.behind),
        foreground: point_delta(previous.foreground, current.foreground),
    }
}

fn max_planes(
    left: SmoothParallaxPlaneTranslations,
    right: SmoothParallaxPlaneTranslations,
) -> SmoothParallaxPlaneTranslations {
    SmoothParallaxPlaneTranslations {
        far: max_point(left.far, right.far),
        mid: max_point(left.mid, right.mid),
        behind: max_point(left.behind, right.behind),
        foreground: max_point(left.foreground, right.foreground),
    }
}

fn max_adjacent_parallax_delta(samples: &[SmoothMotionSample]) -> SmoothParallaxPlaneTranslations {
    samples.windows(2).fold(
        SmoothParallaxPlaneTranslations::default(),
        |maximum, pair| {
            let previous = pair[0].plan.parallax_translations_by_plane();
            let current = pair[1].plan.parallax_translations_by_plane();
            max_planes(maximum, plane_delta(previous, current))
        },
    )
}

fn point_is_nonzero(point: SmoothPoint) -> bool {
    point.x != 0.0 || point.y != 0.0
}

fn points_have_strict_ordering(planes: SmoothParallaxPlaneTranslations) -> bool {
    fn strictly_ordered(far: f32, mid: f32, behind: f32, foreground: f32) -> bool {
        far != 0.0
            && far.abs() < mid.abs()
            && mid.abs() < behind.abs()
            && behind.abs() < foreground.abs()
    }

    strictly_ordered(
        planes.far.x,
        planes.mid.x,
        planes.behind.x,
        planes.foreground.x,
    ) || strictly_ordered(
        planes.far.y,
        planes.mid.y,
        planes.behind.y,
        planes.foreground.y,
    )
}

fn planes_are_within_adjacent_limits(planes: SmoothParallaxPlaneTranslations) -> bool {
    [planes.far, planes.mid, planes.behind, planes.foreground]
        .into_iter()
        .all(|point| point.x <= 0.15 && point.y <= 0.10)
}

fn smooth_motion_window_satisfies_preview_contract(
    start_now: time::OffsetDateTime,
    vm: &WatchViewModel,
    motion: &CompanionMotion,
) -> bool {
    let samples = smooth_motion_samples(start_now, vm, motion);
    let mut classic_snap_anchors = BTreeSet::new();
    let mut saw_nonzero_focus = false;
    let mut saw_nonzero_planes = [false; 4];
    let mut saw_strict_resolved_ordering = false;

    for sample in &samples {
        let plan = &sample.plan;

        classic_snap_anchors.insert((
            plan.pet.classic_snap_anchor.x.round() as i32,
            plan.pet.classic_snap_anchor.y.round() as i32,
        ));
        saw_nonzero_focus |= point_is_nonzero(plan.pet.parallax_focus_offset);

        let planes = plan.parallax_translations_by_plane();
        saw_nonzero_planes[0] |= point_is_nonzero(planes.far);
        saw_nonzero_planes[1] |= point_is_nonzero(planes.mid);
        saw_nonzero_planes[2] |= point_is_nonzero(planes.behind);
        saw_nonzero_planes[3] |= point_is_nonzero(planes.foreground);
        saw_strict_resolved_ordering |= points_have_strict_ordering(planes);

        if plan.layers.iter().any(|layer| {
            matches!(
                layer.motion_binding,
                SmoothLayerMotionBinding::Fixed
                    | SmoothLayerMotionBinding::PetAttached
                    | SmoothLayerMotionBinding::FloorProjected
            ) && point_is_nonzero(layer.parallax_translation)
        }) {
            return false;
        }
    }

    if samples.windows(2).any(|pair| {
        let dx = (pair[1].plan.pet.final_anchor.x - pair[0].plan.pet.final_anchor.x).abs();
        let dy = (pair[1].plan.pet.final_anchor.y - pair[0].plan.pet.final_anchor.y).abs();
        dx >= 1.0 || dy >= 1.0
    }) {
        return false;
    }

    classic_snap_anchors.len() >= 2
        && saw_nonzero_focus
        && saw_nonzero_planes
            .into_iter()
            .all(|saw_nonzero| saw_nonzero)
        && saw_strict_resolved_ordering
        && planes_are_within_adjacent_limits(max_adjacent_parallax_delta(&samples))
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

fn smooth_strip_frame_paths(index: usize) -> PreviewStripFrameFiles {
    PreviewStripFrameFiles {
        text: PathBuf::from(format!("strips/{SMOOTH_MOTION_ID}/frame-{index:03}.txt")),
        cells: PathBuf::from(format!(
            "strips/{SMOOTH_MOTION_ID}/frame-{index:03}.cells.json"
        )),
        pixel: None,
        smooth_motion: Some(PathBuf::from(format!(
            "strips/{SMOOTH_MOTION_ID}/frame-{index:03}.smooth-motion.json"
        ))),
    }
}

fn scene_draw_list_to_preview_frame(
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
    fn pinned_reviewed_motion_start_satisfies_preview_contract() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let motion = crate::round::scene::companion_roam_motion();
        let reviewed_start = reviewed_motion_start_now();

        assert!(smooth_motion_window_satisfies_preview_contract(
            reviewed_start,
            &vm,
            &motion,
        ));
    }

    #[test]
    fn smooth_motion_start_now_prefers_reviewed_start_when_it_passes_contract() {
        let fixed_now = time::macros::datetime!(2026-07-08 18:00:00 UTC);
        let vm = WatchViewModel::fixture_with_habitat_props();
        let motion = crate::round::scene::companion_roam_motion();
        let reviewed_start = reviewed_motion_start_now();

        assert_eq!(
            smooth_motion_start_now(fixed_now, &vm, &motion),
            reviewed_start
        );
    }

    #[test]
    fn smooth_motion_start_now_falls_back_when_reviewed_start_fails_contract() {
        let fixed_now = time::macros::datetime!(2026-07-08 18:00:00 UTC);
        let vm = WatchViewModel::fixture_with_habitat_props();
        let motion = CompanionMotion {
            drift_x_frac: 0.0,
            drift_y_frac: 0.0,
            ..CompanionMotion::default()
        };

        assert_eq!(smooth_motion_start_now(fixed_now, &vm, &motion), fixed_now);
    }

    #[test]
    fn smooth_motion_start_now_returns_first_passing_aligned_search_offset() {
        let fixed_now = time::macros::datetime!(2026-07-08 18:00:00 UTC);
        let vm = WatchViewModel::fixture_with_habitat_props();
        let reviewed_start = reviewed_motion_start_now();
        let mut motion = crate::round::scene::companion_roam_motion();
        motion.drift_period_secs = 4;
        let first_passing_offset_ms = 640;
        let expected = reviewed_start + time::Duration::milliseconds(first_passing_offset_ms);

        assert!(!smooth_motion_window_satisfies_preview_contract(
            reviewed_start,
            &vm,
            &motion,
        ));
        for offset_ms in (MOTION_FRAME_DURATION_MS..first_passing_offset_ms as u64)
            .step_by(MOTION_FRAME_DURATION_MS as usize)
        {
            let candidate = reviewed_start + time::Duration::milliseconds(offset_ms as i64);
            assert!(
                !smooth_motion_window_satisfies_preview_contract(candidate, &vm, &motion),
                "offset {offset_ms}ms should fail before the first passing offset"
            );
        }
        assert!(smooth_motion_window_satisfies_preview_contract(
            expected, &vm, &motion,
        ));
        assert_eq!(smooth_motion_start_now(fixed_now, &vm, &motion), expected);
    }
}
