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
use crate::presentation::smooth::classic_flatten_checksum;
use crate::round::layout::{RoundAperture, SAFE_INNER_RADIUS_RATIO};
use crate::round::scene::{build_round_scene_draw_list, CompanionMotion};
use crate::round::smooth::build_round_smooth_scene_plan;
use crate::tui::view_model::WatchViewModel;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const GRID_COLS: u16 = 52;
const GRID_ROWS: u16 = 52;
const MOTION_FRAME_DURATION_MS: u64 = 160;
const MOTION_FRAME_COUNT: usize = 12;

pub const SMOOTH_BASELINE_ID: &str = "round-smooth-classic-baseline";
pub const SMOOTH_PARITY_ID: &str = "round-smooth-classic-parity";
pub const SMOOTH_MOTION_ID: &str = "round-smooth-motion";

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

    vec![
        PreviewScenarioBundle::from_parts(
            baseline,
            PreviewScenarioKind::Smooth,
            "Review the current Classic cell baseline before comparing Renderer v2 parity.",
            smooth_inputs(ctx, "classic-baseline"),
            Some(smooth_round_metadata(GRID_COLS, GRID_ROWS)),
            vec![
                "Confirm the baseline reads like the current round companion cell fixture."
                    .to_string(),
                "Use this frame as the checksum and visual baseline for smooth parity."
                    .to_string(),
            ],
        ),
        PreviewScenarioBundle::from_parts(
            parity,
            PreviewScenarioKind::Smooth,
            "Review Renderer v2 parity against the same deterministic Classic fixture.",
            smooth_inputs(ctx, "smooth-parity"),
            Some(smooth_round_metadata(GRID_COLS, GRID_ROWS)),
            vec![
                "Confirm the parity frame still reads as the current Glorp companion."
                    .to_string(),
                "Inspect smooth-plan and smooth-parity sidecars for layer coverage and checksum parity."
                    .to_string(),
            ],
        ),
    ]
}

pub fn smooth_strips(ctx: &PreviewRenderContext) -> Vec<PreviewStripBundle> {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let motion = crate::round::scene::companion_roam_motion();
    let motion_start_now = smooth_motion_start_now(ctx.fixed_now, &vm, &motion);
    let mut frames = Vec::with_capacity(MOTION_FRAME_COUNT);
    let mut manifest_frames = Vec::with_capacity(MOTION_FRAME_COUNT);

    for index in 0..MOTION_FRAME_COUNT {
        let elapsed_ms = index as u64 * MOTION_FRAME_DURATION_MS;
        let frame_now = motion_start_now + time::Duration::milliseconds(elapsed_ms as i64);
        let semantic_art_tick_index = elapsed_ms / 250;
        let plan = build_round_smooth_scene_plan(
            &vm, frame_now, GRID_COLS, GRID_ROWS, &motion, elapsed_ms,
        );
        let mut frame = scene_draw_list_to_preview_frame(
            format!("{SMOOTH_MOTION_ID}-frame-{index:03}"),
            format!("Smooth Motion Frame {index:03}"),
            GRID_COLS,
            GRID_ROWS,
            &plan.flatten_classic_cells(),
        );
        frame.contract.smooth_motion = Some(PreviewSmoothMotionArtifact::from_scene_plan(
            SMOOTH_MOTION_ID,
            index as u16,
            elapsed_ms,
            frame_now,
            semantic_art_tick_index,
            &vm,
            &plan,
        ));
        frames.push(frame);
        manifest_frames.push(PreviewStripFrame {
            index: index as u16,
            phase: format!("motion-{index:03}"),
            elapsed_ms: elapsed_ms as u16,
            files: smooth_strip_frame_paths(index),
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
    let search_secs = motion.drift_period_secs.max(1) as i64;
    for offset_secs in 0..search_secs {
        let candidate = fixed_now + time::Duration::seconds(offset_secs);
        if smooth_motion_window_crosses_snapped_anchor(candidate, vm, motion) {
            return candidate;
        }
    }
    fixed_now
}

fn smooth_motion_window_crosses_snapped_anchor(
    start_now: time::OffsetDateTime,
    vm: &WatchViewModel,
    motion: &CompanionMotion,
) -> bool {
    let mut classic_snap_anchors = BTreeSet::new();
    let mut last_final_anchor: Option<(f32, f32)> = None;

    for index in 0..MOTION_FRAME_COUNT {
        let elapsed_ms = index as u64 * MOTION_FRAME_DURATION_MS;
        let frame_now = start_now + time::Duration::milliseconds(elapsed_ms as i64);
        let plan =
            build_round_smooth_scene_plan(vm, frame_now, GRID_COLS, GRID_ROWS, motion, elapsed_ms);

        classic_snap_anchors.insert((
            plan.pet.classic_snap_anchor.x.round() as i32,
            plan.pet.classic_snap_anchor.y.round() as i32,
        ));

        if let Some((last_x, last_y)) = last_final_anchor {
            let dx = (plan.pet.final_anchor.x - last_x).abs();
            let dy = (plan.pet.final_anchor.y - last_y).abs();
            if dx >= 1.0 || dy >= 1.0 {
                return false;
            }
        }
        last_final_anchor = Some((plan.pet.final_anchor.x, plan.pet.final_anchor.y));
    }

    classic_snap_anchors.len() >= 2
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
