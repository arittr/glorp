use crate::dev_preview::export::{
    copy_assets, has_masked_room_artifact, write_cells_json, write_index_html, write_json_artifact,
    write_layout_json, write_manifest, write_review_markdown, write_room_text_frame,
    write_room_text_frame_masked, write_text_frame, ArtifactType, PreviewArtifact,
    PreviewDimensions, PreviewManifest, PreviewMaskRect, PreviewScenario, PreviewScenarioFiles,
    PreviewScenarioKind, PRODUCER, SCHEMA_VERSION,
};
use crate::dev_preview::frame::PreviewFrame;
use crate::dev_preview::habitat_props::{
    habitat_prop_frames, habitat_prop_ids, habitat_prop_motion_phase_timestamps,
};
use crate::dev_preview::output::{commit_output, prepare_output};
use crate::dev_preview::pets::pet_frames;
use crate::dev_preview::watch::watch_frames;
use crate::error::{GlorpError, Result};
use crate::game::evolution::Stage;
use crate::pet::art::stage_label;
use crate::pet::generation::Species;
use crate::tui::render_context::{RenderContext, WatchClock};
use crate::tui::style::ColorCapability;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewSelection {
    All,
    Watch,
    Pets,
    Props,
    Animation,
    Round,
}

pub struct PreviewRenderContext {
    pub fixed_now: OffsetDateTime,
    pub render: RenderContext,
}

pub struct PreviewScenarioBundle {
    pub frame: PreviewFrame,
    pub scenario: PreviewScenario,
}

impl PreviewScenarioBundle {
    pub fn from_frame(frame: PreviewFrame, ctx: &PreviewRenderContext) -> Self {
        let scenario = scenario_metadata(&frame, ctx);
        Self { frame, scenario }
    }
}

impl PreviewRenderContext {
    pub fn deterministic() -> Self {
        let fixed_now = OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap();
        Self {
            fixed_now,
            render: RenderContext::with_clock(
                ColorCapability::Truecolor,
                WatchClock::fixed(fixed_now),
            ),
        }
    }
}

pub fn generate_preview_bundle(out: &Path, selection: PreviewSelection) -> Result<()> {
    let ctx = PreviewRenderContext::deterministic();
    let prepared = prepare_output(out)?;
    let staging_dir = prepared.staging_dir.clone();
    let frames_dir = staging_dir.join("frames");
    let scratch_dir = staging_dir.join(".scratch");

    fs::create_dir_all(&frames_dir)?;
    fs::create_dir_all(&scratch_dir)?;

    let mut frames = Vec::new();
    let mut strips = Vec::new();
    match selection {
        PreviewSelection::All => {
            frames.extend(watch_frames(&ctx, &scratch_dir)?);
            frames.extend(habitat_prop_frames(&ctx, &scratch_dir)?);
            frames.extend(pet_frames(&ctx)?);
            frames.extend(crate::dev_preview::round::round_frames(&ctx));
            strips.push(crate::dev_preview::strips::scene_strip_smoke());
            strips.extend(crate::dev_preview::strips::scene_strips(&ctx));
        }
        PreviewSelection::Watch => frames.extend(watch_frames(&ctx, &scratch_dir)?),
        PreviewSelection::Pets => frames.extend(pet_frames(&ctx)?),
        PreviewSelection::Props => frames.extend(habitat_prop_frames(&ctx, &scratch_dir)?),
        PreviewSelection::Animation => {
            strips.push(crate::dev_preview::strips::scene_strip_smoke());
            strips.extend(crate::dev_preview::strips::scene_strips(&ctx));
        }
        PreviewSelection::Round => frames.extend(crate::dev_preview::round::round_frames(&ctx)),
    }

    let bundles = frames
        .into_iter()
        .map(|frame| PreviewScenarioBundle::from_frame(frame, &ctx))
        .collect::<Vec<_>>();
    let frames = bundles
        .iter()
        .map(|bundle| bundle.frame.clone())
        .collect::<Vec<_>>();
    let masked_room_masks = masked_room_masks_for_species_dialect_pairs(&frames);

    for frame in &frames {
        write_text_frame(&staging_dir.join(text_path(frame)), frame)?;
        write_cells_json(&staging_dir.join(cells_path(frame)), frame)?;
        if let Some(layout) = &frame.layout {
            write_layout_json(&staging_dir.join(layout_path(frame)), layout)?;
            if layout.targets.contains_key("watch.room.effect") {
                write_room_text_frame(
                    &staging_dir.join(room_text_path(frame)),
                    frame,
                    "watch.room.effect",
                )?;
            }
        }
        if let Some(layout) = &frame.layout {
            if layout.targets.contains_key("watch.room.effect") {
                if let Some(masks) = masked_room_masks.get(&frame.id) {
                    write_room_text_frame_masked(
                        &staging_dir.join(room_masked_text_path(frame)),
                        frame,
                        "watch.room.effect",
                        masks,
                    )?;
                }
            }
        }
        if let Some(scene) = &frame.contract.scene {
            write_json_artifact(&staging_dir.join(scene_path(frame)), scene)?;
        }
        if let Some(round_layout) = &frame.contract.round_layout {
            write_json_artifact(&staging_dir.join(round_layout_path(frame)), round_layout)?;
        }
        if let Some(round_commands) = &frame.contract.round_commands {
            write_json_artifact(
                &staging_dir.join(round_commands_path(frame)),
                round_commands,
            )?;
        }
    }

    for strip in &strips {
        fs::create_dir_all(staging_dir.join("strips").join(&strip.manifest.id))?;
        for (frame, manifest_frame) in strip.frames.iter().zip(&strip.manifest.frames) {
            write_text_frame(&staging_dir.join(&manifest_frame.files.text), frame)?;
            write_cells_json(&staging_dir.join(&manifest_frame.files.cells), frame)?;
        }
    }

    let generated_at = format_rfc3339(OffsetDateTime::now_utc())?;
    let scenarios = bundles
        .iter()
        .map(|bundle| bundle.scenario.clone())
        .collect();
    let manifest = PreviewManifest {
        schema_version: SCHEMA_VERSION,
        producer: PRODUCER,
        glorp_version: env!("CARGO_PKG_VERSION"),
        generated_at,
        scenarios,
        strips: strips.iter().map(|strip| strip.manifest.clone()).collect(),
        artifacts: artifacts_for_frames(&frames)
            .into_iter()
            .chain(artifacts_for_strips(&strips))
            .collect(),
    };

    write_index_html(
        &staging_dir.join("index.html"),
        &frames,
        &strips,
        &manifest.generated_at,
    )?;
    write_review_markdown(&staging_dir.join("review.md"), &manifest)?;
    copy_assets(&staging_dir)?;
    write_manifest(&staging_dir.join("manifest.json"), &manifest)?;

    fs::remove_dir_all(&scratch_dir)?;
    commit_output(prepared)
}

fn scenario_metadata(frame: &PreviewFrame, ctx: &PreviewRenderContext) -> PreviewScenario {
    let (kind, intent, inputs, review_prompts) = match frame.id.as_str() {
        "watch-wide-normal" => (
            PreviewScenarioKind::Watch,
            "Review the primary healthy watch layout at the standard wide terminal size.",
            BTreeMap::from([
                (
                    "fixed_now".to_string(),
                    Value::String(format_rfc3339_lossy(ctx.fixed_now)),
                ),
                (
                    "color_capability".to_string(),
                    Value::String(color_capability_name(ctx.render.color_capability).to_string()),
                ),
                (
                    "fixture".to_string(),
                    Value::String("seeded-pet-state-and-usage-sqlite".to_string()),
                ),
                ("species".to_string(), Value::String("fuzz".to_string())),
                ("stage".to_string(), Value::String("s4".to_string())),
                ("mood".to_string(), Value::String("content".to_string())),
                ("tick".to_string(), json!(ctx.fixed_now.unix_timestamp())),
                ("habitat_props".to_string(), preview_habitat_props()),
                ("terminal_width".to_string(), json!(120)),
                ("terminal_height".to_string(), json!(32)),
            ]),
            vec![
                "Check the two-column layout hierarchy and spacing.".to_string(),
                "Confirm source health and recent activity are readable.".to_string(),
            ],
        ),
        "watch-tall-wide" => (
            PreviewScenarioKind::Watch,
            "Review the healthy watch layout on a tall wide terminal with the pet scene absorbing vertical slack.",
            BTreeMap::from([
                (
                    "fixed_now".to_string(),
                    Value::String(format_rfc3339_lossy(ctx.fixed_now)),
                ),
                (
                    "color_capability".to_string(),
                    Value::String(color_capability_name(ctx.render.color_capability).to_string()),
                ),
                (
                    "fixture".to_string(),
                    Value::String("seeded-pet-state-and-usage-sqlite".to_string()),
                ),
                ("species".to_string(), Value::String("fuzz".to_string())),
                ("stage".to_string(), Value::String("s4".to_string())),
                ("mood".to_string(), Value::String("content".to_string())),
                ("tick".to_string(), json!(ctx.fixed_now.unix_timestamp())),
                ("habitat_props".to_string(), preview_habitat_props()),
                ("terminal_width".to_string(), json!(180)),
                ("terminal_height".to_string(), json!(50)),
            ]),
            vec![
                "Check that width is centered while the frame uses the full terminal height."
                    .to_string(),
                "Confirm the pet scene absorbs vertical slack without crowding usage details."
                    .to_string(),
            ],
        ),
        "watch-compact-normal" => (
            PreviewScenarioKind::Watch,
            "Review the healthy watch layout at the compact terminal size.",
            BTreeMap::from([
                (
                    "fixed_now".to_string(),
                    Value::String(format_rfc3339_lossy(ctx.fixed_now)),
                ),
                (
                    "color_capability".to_string(),
                    Value::String(color_capability_name(ctx.render.color_capability).to_string()),
                ),
                (
                    "fixture".to_string(),
                    Value::String("seeded-pet-state-and-usage-sqlite".to_string()),
                ),
                ("species".to_string(), Value::String("fuzz".to_string())),
                ("stage".to_string(), Value::String("s4".to_string())),
                ("mood".to_string(), Value::String("content".to_string())),
                ("tick".to_string(), json!(ctx.fixed_now.unix_timestamp())),
                ("habitat_props".to_string(), preview_habitat_props()),
                ("terminal_width".to_string(), json!(72)),
                ("terminal_height".to_string(), json!(24)),
            ]),
            vec![
                "Check compact stacking and truncation behavior.".to_string(),
                "Confirm critical pet and usage details remain visible.".to_string(),
            ],
        ),
        id if id.starts_with("watch-liveliness-") => (
            PreviewScenarioKind::Watch,
            "Review deterministic live-scene profile inputs before visual liveliness rendering consumes them.",
            life_profile_inputs_for_frame(id, frame, ctx),
            vec![
                "Confirm the frame has a truthful life_profile input contract.".to_string(),
                "Use the cells and layout artifacts as Task 7 proof fixtures.".to_string(),
            ],
        ),
        id if id.starts_with("watch-daycontext-") => (
            PreviewScenarioKind::Watch,
            "Review deterministic day-phase and rhythm inputs before the watch scene renders them.",
            day_context_inputs_for_frame(id, frame, ctx),
            vec![
                "Confirm the frame has a truthful day_context input contract.".to_string(),
                "Use the cells artifact as Task 14 proof fixtures.".to_string(),
            ],
        ),
        "pet-species-stage" => (
            PreviewScenarioKind::PetMatrix,
            "Review every Slice 1 pet species across all seven growth stages.",
            BTreeMap::from([
                ("mood".to_string(), Value::String("content".to_string())),
                ("tick".to_string(), json!(0)),
                ("species_count".to_string(), json!(6)),
                ("stage_count".to_string(), json!(7)),
                (
                    "species".to_string(),
                    json!(Species::all()
                        .into_iter()
                        .map(Species::as_str)
                        .collect::<Vec<_>>()),
                ),
                ("stages".to_string(), json!(stage_inputs())),
                (
                    "color_capability".to_string(),
                    Value::String(color_capability_name(ctx.render.color_capability).to_string()),
                ),
            ]),
            vec![
                "Check species silhouettes remain distinct across stages.".to_string(),
                "Confirm stage labels and role colors line up with watch pet styling.".to_string(),
            ],
        ),
        "pet-species-stage-flat" => (
            PreviewScenarioKind::PetMatrix,
            "Review every Slice 1 pet species across all seven growth stages on a flat (non-truecolor) terminal.",
            BTreeMap::from([
                ("mood".to_string(), Value::String("content".to_string())),
                ("tick".to_string(), json!(0)),
                ("species_count".to_string(), json!(6)),
                ("stage_count".to_string(), json!(7)),
                (
                    "species".to_string(),
                    json!(Species::all()
                        .into_iter()
                        .map(Species::as_str)
                        .collect::<Vec<_>>()),
                ),
                ("stages".to_string(), json!(stage_inputs())),
                (
                    "color_capability".to_string(),
                    Value::String(color_capability_name(ColorCapability::Flat).to_string()),
                ),
            ]),
            vec![
                "Check species silhouettes remain distinct across stages.".to_string(),
                "Confirm the flat ANSI fallback still distinguishes species.".to_string(),
            ],
        ),
        "pet-glitch-live-states" => (
            PreviewScenarioKind::PetMatrix,
            "Review Glitch pet expression and elder silhouette under live work-state accents.",
            BTreeMap::from([
                ("species".to_string(), Value::String("glitch".to_string())),
                ("color_capability".to_string(), Value::String("truecolor".to_string())),
                (
                    "states".to_string(),
                    json!([
                        {
                            "stage": "s4",
                            "mood": "content",
                            "work_accent": "none"
                        },
                        {
                            "stage": "s4",
                            "mood": "ecstatic",
                            "work_accent": "none"
                        },
                        {
                            "stage": "s4",
                            "mood": "ecstatic",
                            "work_accent": "dreamy"
                        },
                        {
                            "stage": "s6",
                            "mood": "happy",
                            "work_accent": "focused"
                        }
                    ]),
                ),
            ]),
            vec![
                "Confirm ecstatic Glitch never reads as sleepy when cache-mist work is active.".to_string(),
                "Compare S4 and S6 silhouettes for visible elder progression.".to_string(),
            ],
        ),
        "habitat-props-catalog" => (
            PreviewScenarioKind::HabitatProps,
            "Review every habitat prop in isolated tanks across three deterministic motion phases.",
            BTreeMap::from([
                (
                    "fixed_now".to_string(),
                    Value::String(format_rfc3339_lossy(ctx.fixed_now)),
                ),
                (
                    "color_capability".to_string(),
                    Value::String(color_capability_name(ctx.render.color_capability).to_string()),
                ),
                ("prop_count".to_string(), json!(habitat_prop_ids().len())),
                ("props".to_string(), json!(habitat_prop_ids())),
                (
                    "motion_phases".to_string(),
                    json!(habitat_prop_motion_phase_timestamps(ctx)),
                ),
            ]),
            vec![
                "Check that each prop reads as a discrete object rather than ambient texture."
                    .to_string(),
                "Compare the three phase columns for subtle motion without distracting jumps."
                    .to_string(),
            ],
        ),
        id if id.starts_with("watch-habitat-") => (
            PreviewScenarioKind::HabitatProps,
            "Review habitat props inside realistic watch frames at different earned-state densities.",
            BTreeMap::from([
                (
                    "fixed_now".to_string(),
                    Value::String(format_rfc3339_lossy(ctx.fixed_now)),
                ),
                (
                    "color_capability".to_string(),
                    Value::String(color_capability_name(ctx.render.color_capability).to_string()),
                ),
                ("props".to_string(), json!(habitat_prop_ids())),
                ("terminal_width".to_string(), json!(frame.width)),
                ("terminal_height".to_string(), json!(frame.height)),
            ]),
            vec![
                "Check whether props feel integrated with the pet scene instead of pasted on."
                    .to_string(),
                "Compare early, lived-in, and full states for density and object legibility."
                    .to_string(),
            ],
        ),
        id if id.starts_with("watch-activity-identity-") => (
            PreviewScenarioKind::Watch,
            "Review activity-identity driven watch layout with multi-source or unknown-source usage.",
            activity_identity_inputs_for_frame(id, frame, ctx),
            vec![
                "Confirm the Today panel shows dynamic source rows and an 'other' row if needed.".to_string(),
                "Check that source labels are colored deterministically, including unknown sources.".to_string(),
                "Verify the manifest records source mix and activity profile intent without raw payloads.".to_string(),
            ],
        ),
        id if id.starts_with("watch-species-dialect-") => (
            PreviewScenarioKind::Watch,
            "Review species room dialect rendering under identical earned-room inputs.",
            species_dialect_inputs_for_frame(id, frame),
            vec![
                "Compare paired .room.txt crops first to confirm the same earned room is present.".to_string(),
                "Compare paired .room-masked.txt crops for species dialect differences outside pet art and shared props.".to_string(),
                "Confirm earned props remain recognizable and species changes the room texture, not the prop identity.".to_string(),
            ],
        ),
        id if id.starts_with("room-") => (
            PreviewScenarioKind::Watch,
            "Review deterministic alive room inputs and rendered output.",
            frame.extra_inputs.clone(),
            vec![
                "Confirm the frame shows the expected primary biome.".to_string(),
                "Check room symbol distribution across upper-air, floor, and anchor zones."
                    .to_string(),
            ],
        ),
        id if id.starts_with("round-") => (
            PreviewScenarioKind::Round,
            "Review round macOS companion preview with aperture masking and privacy metadata.",
            BTreeMap::from([
                (
                    "color_capability".to_string(),
                    Value::String(color_capability_name(ctx.render.color_capability).to_string()),
                ),
                ("fixture".to_string(), Value::String("seeded-pet-state-and-usage-sqlite".to_string())),
                ("privacy_source_names_visible".to_string(), Value::Bool(false)),
                ("privacy_exact_counts_visible".to_string(), Value::Bool(false)),
                ("privacy_diagnostic_text_visible".to_string(), Value::Bool(false)),
            ]),
            vec![
                "Confirm the circular aperture masks the frame corners.".to_string(),
                "Check that dashboard labels and source diagnostics are not visible.".to_string(),
                "Verify privacy metadata records all visibility flags as false.".to_string(),
            ],
        ),
        _ => (
            PreviewScenarioKind::Watch,
            "Review this generated preview frame.",
            BTreeMap::new(),
            vec!["Inspect the generated frame for obvious regressions.".to_string()],
        ),
    };

    PreviewScenario {
        id: frame.id.clone(),
        kind,
        title: frame.title.clone(),
        intent: intent.to_string(),
        dimensions: PreviewDimensions {
            width: frame.width,
            height: frame.height,
        },
        files: PreviewScenarioFiles {
            text: text_path(frame),
            cells: cells_path(frame),
            layout: frame.layout.as_ref().map(|_| layout_path(frame)),
            room_text: frame.layout.as_ref().and_then(|layout| {
                if layout.targets.contains_key("watch.room.effect") {
                    Some(room_text_path(frame))
                } else {
                    None
                }
            }),
            room_masked_text: frame.layout.as_ref().and_then(|layout| {
                if layout.targets.contains_key("watch.room.effect")
                    && has_masked_room_artifact(&frame.id)
                {
                    Some(room_masked_text_path(frame))
                } else {
                    None
                }
            }),
            scene: frame.contract.scene.as_ref().map(|_| scene_path(frame)),
            round_layout: frame
                .contract
                .round_layout
                .as_ref()
                .map(|_| round_layout_path(frame)),
            round_commands: frame
                .contract
                .round_commands
                .as_ref()
                .map(|_| round_commands_path(frame)),
        },
        inputs,
        round: if frame.id.starts_with("round-") {
            let aperture = crate::round::layout::RoundAperture::new(frame.width, frame.height);
            Some(crate::dev_preview::export::PreviewRoundMetadata {
                target_renderer: "preview-cells",
                aperture: crate::dev_preview::export::PreviewRoundAperture {
                    shape: "circle",
                    center_x: aperture.center_x,
                    center_y: aperture.center_y,
                    radius: aperture.radius,
                    safe_inner_radius: aperture.radius
                        * crate::round::layout::SAFE_INNER_RADIUS_RATIO,
                    transparent_outside_aperture: true,
                },
                privacy: crate::dev_preview::export::PreviewRoundPrivacy {
                    source_names_visible: false,
                    exact_counts_visible: false,
                    diagnostic_text_visible: false,
                },
            })
        } else {
            None
        },
        review_prompts,
    }
}

fn artifacts_for_frames(frames: &[PreviewFrame]) -> Vec<PreviewArtifact> {
    let mut artifacts = Vec::new();
    for frame in frames {
        artifacts.push(PreviewArtifact {
            id: frame.id.clone(),
            title: format!("{} Text", frame.title),
            artifact_type: ArtifactType::Text,
            path: text_path(frame),
            width: Some(frame.width),
            height: Some(frame.height),
        });
        artifacts.push(PreviewArtifact {
            id: format!("{}-cells", frame.id),
            title: format!("{} Cells", frame.title),
            artifact_type: ArtifactType::Cells,
            path: cells_path(frame),
            width: Some(frame.width),
            height: Some(frame.height),
        });
        if frame.layout.is_some() {
            artifacts.push(PreviewArtifact {
                id: format!("{}-layout", frame.id),
                title: format!("{} Layout", frame.title),
                artifact_type: ArtifactType::Layout,
                path: layout_path(frame),
                width: Some(frame.width),
                height: Some(frame.height),
            });
        }
        if frame
            .layout
            .as_ref()
            .is_some_and(|l| l.targets.contains_key("watch.room.effect"))
        {
            artifacts.push(PreviewArtifact {
                id: format!("{}-room", frame.id),
                title: format!("{} Room", frame.title),
                artifact_type: ArtifactType::Text,
                path: room_text_path(frame),
                width: Some(frame.width),
                height: Some(frame.height),
            });
        }
        if frame
            .layout
            .as_ref()
            .is_some_and(|l| l.targets.contains_key("watch.room.effect"))
            && has_masked_room_artifact(&frame.id)
        {
            artifacts.push(PreviewArtifact {
                id: format!("{}-room-masked", frame.id),
                title: format!("{} Masked Room", frame.title),
                artifact_type: ArtifactType::Text,
                path: room_masked_text_path(frame),
                width: Some(frame.width),
                height: Some(frame.height),
            });
        }
        if frame.contract.scene.is_some() {
            artifacts.push(PreviewArtifact {
                id: format!("{}-scene", frame.id),
                title: format!("{} Scene", frame.title),
                artifact_type: ArtifactType::Scene,
                path: scene_path(frame),
                width: None,
                height: None,
            });
        }
        if frame.contract.round_layout.is_some() {
            artifacts.push(PreviewArtifact {
                id: format!("{}-round-layout", frame.id),
                title: format!("{} Round Layout", frame.title),
                artifact_type: ArtifactType::RoundLayout,
                path: round_layout_path(frame),
                width: None,
                height: None,
            });
        }
        if frame.contract.round_commands.is_some() {
            artifacts.push(PreviewArtifact {
                id: format!("{}-round-commands", frame.id),
                title: format!("{} Round Commands", frame.title),
                artifact_type: ArtifactType::RoundCommands,
                path: round_commands_path(frame),
                width: None,
                height: None,
            });
        }
    }
    artifacts.push(PreviewArtifact {
        id: "index".to_string(),
        title: "Preview Index".to_string(),
        artifact_type: ArtifactType::Html,
        path: PathBuf::from("index.html"),
        width: None,
        height: None,
    });
    artifacts.push(PreviewArtifact {
        id: "review".to_string(),
        title: "Review Checklist".to_string(),
        artifact_type: ArtifactType::Review,
        path: PathBuf::from("review.md"),
        width: None,
        height: None,
    });
    artifacts.push(PreviewArtifact {
        id: "preview-css".to_string(),
        title: "Preview CSS".to_string(),
        artifact_type: ArtifactType::Asset,
        path: PathBuf::from("assets/preview.css"),
        width: None,
        height: None,
    });
    artifacts.push(PreviewArtifact {
        id: "preview-js".to_string(),
        title: "Preview JS".to_string(),
        artifact_type: ArtifactType::Asset,
        path: PathBuf::from("assets/preview.js"),
        width: None,
        height: None,
    });
    artifacts
}

fn artifacts_for_strips(
    strips: &[crate::dev_preview::strips::PreviewStripBundle],
) -> Vec<PreviewArtifact> {
    let mut artifacts = Vec::new();
    for strip in strips {
        for (index, (frame, manifest_frame)) in
            strip.frames.iter().zip(&strip.manifest.frames).enumerate()
        {
            artifacts.push(PreviewArtifact {
                id: format!("{}-frame-{index:03}", strip.manifest.id),
                title: format!("{} Frame {index:03} Text", strip.manifest.title),
                artifact_type: ArtifactType::Text,
                path: manifest_frame.files.text.clone(),
                width: Some(frame.width),
                height: Some(frame.height),
            });
            artifacts.push(PreviewArtifact {
                id: format!("{}-frame-{index:03}-cells", strip.manifest.id),
                title: format!("{} Frame {index:03} Cells", strip.manifest.title),
                artifact_type: ArtifactType::Cells,
                path: manifest_frame.files.cells.clone(),
                width: Some(frame.width),
                height: Some(frame.height),
            });
        }
    }
    artifacts
}

fn preview_habitat_props() -> Value {
    json!([
        "codex_signal_lamp",
        "heavy_session_planter",
        "token_pebble_25k",
        "token_shell_100k"
    ])
}

fn life_profile_inputs_for_frame(
    id: &str,
    frame: &PreviewFrame,
    ctx: &PreviewRenderContext,
) -> BTreeMap<String, Value> {
    let midday = ctx.fixed_now + time::Duration::hours(4);
    let evening = ctx.fixed_now + time::Duration::hours(10);
    let (
        activity_level,
        burst_level,
        source_accent,
        weather,
        prop_reactions,
        color_capability,
        calm_mode,
        freshness,
        fixed_now,
    ) = match id {
        "watch-liveliness-s6-idle-dawn" => (
            0.0,
            0.0,
            None,
            "clear",
            json!([]),
            ColorCapability::Truecolor,
            false,
            "cold-start",
            ctx.fixed_now,
        ),
        "watch-liveliness-s6-warm-midday" => (
            0.68,
            0.24,
            Some("balanced"),
            "mixed",
            json!([{
                "prop_id": "codex_signal_lamp",
                "intensity": 0.35,
                "kind": "glow"
            }]),
            ColorCapability::Truecolor,
            false,
            "live",
            midday,
        ),
        "watch-liveliness-s6-hot-midday"
        | "watch-liveliness-compact-s6-hot"
        | "watch-liveliness-flat-s6-hot" => (
            1.56,
            1.22,
            Some("codex"),
            "output-sparks",
            json!([
                {
                    "prop_id": "codex_signal_lamp",
                    "intensity": 0.9,
                    "kind": "pulse"
                },
                {
                    "prop_id": "heavy_session_planter",
                    "intensity": 0.72,
                    "kind": "bloom"
                }
            ]),
            if id == "watch-liveliness-flat-s6-hot" {
                ColorCapability::Flat
            } else {
                ColorCapability::Truecolor
            },
            false,
            "live",
            midday,
        ),
        "watch-liveliness-calm-mode-s6-hot" => (
            1.56,
            1.22,
            Some("codex"),
            "output-sparks",
            json!([
                {
                    "prop_id": "codex_signal_lamp",
                    "intensity": 0.9,
                    "kind": "pulse"
                },
                {
                    "prop_id": "heavy_session_planter",
                    "intensity": 0.72,
                    "kind": "bloom"
                }
            ]),
            ColorCapability::Truecolor,
            true,
            "live",
            midday,
        ),
        "watch-liveliness-s6-cooling-evening" => (
            0.38,
            0.0,
            Some("claude"),
            "cache-mist",
            json!([{
                "prop_id": "token_shell_100k",
                "intensity": 0.28,
                "kind": "glow"
            }]),
            ColorCapability::Truecolor,
            false,
            "backfill",
            evening,
        ),
        _ => (
            0.0,
            0.0,
            None,
            "clear",
            json!([]),
            ctx.render.color_capability,
            false,
            "unknown",
            ctx.fixed_now,
        ),
    };

    BTreeMap::from([
        (
            "fixed_now".to_string(),
            Value::String(format_rfc3339_lossy(fixed_now)),
        ),
        ("tick".to_string(), json!(fixed_now.unix_timestamp())),
        ("terminal_width".to_string(), json!(frame.width)),
        ("terminal_height".to_string(), json!(frame.height)),
        (
            "life_profile".to_string(),
            json!({
                "activity_level": activity_level,
                "burst_level": burst_level,
                "source_accent": source_accent,
                "weather": weather,
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": prop_reactions,
                "color_capability": color_capability_name(color_capability),
                "calm_mode": calm_mode,
                "freshness": freshness
            }),
        ),
    ])
}

fn day_context_inputs_for_frame(
    id: &str,
    frame: &PreviewFrame,
    ctx: &PreviewRenderContext,
) -> BTreeMap<String, Value> {
    let fixed_now = ctx.fixed_now;
    let (
        day_phase,
        phase_started_at_utc,
        phase_ends_at_utc,
        asleep,
        sleep_onset_utc,
        wake_resume,
        blend,
        life_profile,
        extras,
    ) = match id {
        "watch-daycontext-night-asleep" => (
            "night",
            fixed_now - time::Duration::hours(2),
            fixed_now + time::Duration::hours(6),
            true,
            Some(fixed_now - time::Duration::minutes(25)),
            None,
            1.0,
            json!({
                "activity_level": 0.0,
                "burst_level": 0.0,
                "source_accent": None::<&str>,
                "weather": "clear",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([]),
                "color_capability": "truecolor",
                "calm_mode": true,
                "freshness": "live"
            }),
            json!({}),
        ),
        "watch-daycontext-dawn-crossing" => (
            "dawn",
            fixed_now - time::Duration::minutes(10),
            fixed_now + time::Duration::minutes(20),
            false,
            None,
            None,
            0.33,
            json!({
                "activity_level": 0.68,
                "burst_level": 0.24,
                "source_accent": Some("balanced"),
                "weather": "mixed",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([{
                    "prop_id": "codex_signal_lamp",
                    "intensity": 0.35,
                    "kind": "glow"
                }]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({}),
        ),
        "watch-daycontext-night-wake-catchup" => (
            "night",
            fixed_now - time::Duration::hours(2),
            fixed_now + time::Duration::hours(6),
            false,
            None,
            Some((
                fixed_now - time::Duration::hours(3),
                fixed_now - time::Duration::minutes(2),
            )),
            1.0,
            json!({
                "activity_level": 0.38,
                "burst_level": 0.0,
                "source_accent": Some("claude"),
                "weather": "cache-mist",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([{
                    "prop_id": "token_shell_100k",
                    "intensity": 0.28,
                    "kind": "glow"
                }]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "backfill"
            }),
            json!({}),
        ),
        "watch-daycontext-hatch-at-night" => (
            "night",
            fixed_now - time::Duration::hours(2),
            fixed_now + time::Duration::hours(6),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.0,
                "burst_level": 0.0,
                "source_accent": None::<&str>,
                "weather": "clear",
                "stage": "s0",
                "species": "fuzz",
                "prop_reactions": json!([]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "cold-start"
            }),
            json!({}),
        ),
        "watch-daycontext-dream-night" => (
            "night",
            fixed_now - time::Duration::hours(3),
            fixed_now + time::Duration::hours(5),
            true,
            Some(fixed_now - time::Duration::hours(1)),
            None,
            1.0,
            json!({
                "activity_level": 0.0,
                "burst_level": 0.0,
                "source_accent": None::<&str>,
                "weather": "clear",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([]),
                "color_capability": "truecolor",
                "calm_mode": true,
                "freshness": "live"
            }),
            json!({
                "yesterday": {
                    "ratio": 1.6,
                    "dominant_shape": "cache-mist"
                },
                "date_seed": 7,
                "mature": true
            }),
        ),
        "watch-daycontext-heavy-day-evening" => (
            "dusk",
            fixed_now - time::Duration::hours(1),
            fixed_now + time::Duration::hours(2),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.38,
                "burst_level": 0.0,
                "source_accent": Some("claude"),
                "weather": "cache-mist",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([{
                    "prop_id": "token_shell_100k",
                    "intensity": 0.28,
                    "kind": "glow"
                }]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "backfill"
            }),
            json!({
                "today_ratio": 1.7,
                "tiredness": 0.85,
                "mature": true
            }),
        ),
        "watch-daycontext-light-day-morning" => (
            "dawn",
            fixed_now - time::Duration::minutes(40),
            fixed_now + time::Duration::minutes(80),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.0,
                "burst_level": 0.0,
                "source_accent": None::<&str>,
                "weather": "clear",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "today_ratio": 0.02,
                "yesterday": {
                    "ratio": 0.04,
                    "dominant_shape": null
                },
                "mature": true
            }),
        ),
        "watch-daycontext-weekend-midday" => (
            "day",
            fixed_now - time::Duration::hours(3),
            fixed_now + time::Duration::hours(5),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.0,
                "burst_level": 0.0,
                "source_accent": None::<&str>,
                "weather": "clear",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "is_weekend": true,
                "weekend_share": 0.05,
                "today_ratio": 0.3,
                "mature": true
            }),
        ),
        "watch-daycontext-climate-cache-week" => (
            "day",
            fixed_now - time::Duration::hours(3),
            fixed_now + time::Duration::hours(5),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.0,
                "burst_level": 0.0,
                "source_accent": None::<&str>,
                "weather": "clear",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "climate": "cache-mist",
                "today_ratio": 0.5,
                "mature": true
            }),
        ),
        "watch-daycontext-prop-resonance-planter" => (
            "day",
            fixed_now - time::Duration::hours(3),
            fixed_now + time::Duration::hours(5),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.0,
                "burst_level": 0.0,
                "source_accent": None::<&str>,
                "weather": "clear",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "yesterday": {
                    "ratio": 1.9,
                    "dominant_shape": "output-sparks"
                },
                "today_ratio": 0.4,
                "mature": true
            }),
        ),
        "watch-daycontext-midnight-mid-session" => (
            "night",
            fixed_now - time::Duration::hours(2),
            fixed_now + time::Duration::hours(6),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.68,
                "burst_level": 0.24,
                "source_accent": Some("balanced"),
                "weather": "mixed",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([{
                    "prop_id": "codex_signal_lamp",
                    "intensity": 0.35,
                    "kind": "glow"
                }]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "today_ratio": 0.03,
                "tiredness": 0.45,
                "yesterday": {
                    "ratio": 1.3,
                    "dominant_shape": "mixed"
                },
                "mature": true
            }),
        ),
        "watch-daycontext-dawn-fresh" => (
            "dawn",
            fixed_now - time::Duration::minutes(40),
            fixed_now + time::Duration::minutes(80),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.38,
                "burst_level": 0.0,
                "source_accent": None::<&str>,
                "weather": "clear",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "today_ratio": 0.05,
                "tiredness": 0.0,
                "mature": true
            }),
        ),
        "watch-daycontext-dusk-heavy" => (
            "dusk",
            fixed_now - time::Duration::hours(1),
            fixed_now + time::Duration::hours(2),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.38,
                "burst_level": 0.0,
                "source_accent": Some("claude"),
                "weather": "cache-mist",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([{
                    "prop_id": "token_shell_100k",
                    "intensity": 0.28,
                    "kind": "glow"
                }]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "backfill"
            }),
            json!({
                "today_ratio": 1.6,
                "tiredness": 0.8,
                "mature": true
            }),
        ),
        "watch-daycontext-night-quiet" => (
            "night",
            fixed_now - time::Duration::hours(2),
            fixed_now + time::Duration::hours(6),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.0,
                "burst_level": 0.0,
                "source_accent": None::<&str>,
                "weather": "clear",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "today_ratio": 0.1,
                "tiredness": 0.3,
                "mature": true
            }),
        ),
        "watch-daycontext-work-output-sparks" => (
            "day",
            fixed_now - time::Duration::hours(3),
            fixed_now + time::Duration::hours(5),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.68,
                "burst_level": 0.24,
                "source_accent": Some("balanced"),
                "weather": "output-sparks",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([{
                    "prop_id": "codex_signal_lamp",
                    "intensity": 0.35,
                    "kind": "glow"
                }]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "today_ratio": 0.6,
                "tiredness": 0.1,
                "mature": true
            }),
        ),
        "watch-daycontext-work-reasoning-pulse" => (
            "day",
            fixed_now - time::Duration::hours(3),
            fixed_now + time::Duration::hours(5),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.68,
                "burst_level": 0.24,
                "source_accent": Some("balanced"),
                "weather": "reasoning-pulse",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([{
                    "prop_id": "codex_signal_lamp",
                    "intensity": 0.35,
                    "kind": "glow"
                }]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "today_ratio": 0.6,
                "tiredness": 0.1,
                "mature": true
            }),
        ),
        "watch-daycontext-work-cache-mist" => (
            "day",
            fixed_now - time::Duration::hours(3),
            fixed_now + time::Duration::hours(5),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.38,
                "burst_level": 0.0,
                "source_accent": Some("claude"),
                "weather": "cache-mist",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([{
                    "prop_id": "token_shell_100k",
                    "intensity": 0.28,
                    "kind": "glow"
                }]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "backfill"
            }),
            json!({
                "today_ratio": 0.6,
                "tiredness": 0.1,
                "mature": true
            }),
        ),
        "watch-daycontext-work-mixed" => (
            "day",
            fixed_now - time::Duration::hours(3),
            fixed_now + time::Duration::hours(5),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.68,
                "burst_level": 0.24,
                "source_accent": Some("balanced"),
                "weather": "mixed",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([{
                    "prop_id": "codex_signal_lamp",
                    "intensity": 0.35,
                    "kind": "glow"
                }]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "today_ratio": 0.6,
                "tiredness": 0.1,
                "mature": true
            }),
        ),
        "watch-daycontext-work-clear" => (
            "day",
            fixed_now - time::Duration::hours(3),
            fixed_now + time::Duration::hours(5),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.0,
                "burst_level": 0.0,
                "source_accent": None::<&str>,
                "weather": "clear",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "today_ratio": 0.6,
                "tiredness": 0.1,
                "mature": true
            }),
        ),
        _ => (
            "day",
            fixed_now,
            fixed_now,
            false,
            None,
            None,
            0.0,
            json!({
                "activity_level": 0.0,
                "burst_level": 0.0,
                "source_accent": None::<&str>,
                "weather": "clear",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([]),
                "color_capability": color_capability_name(ctx.render.color_capability),
                "calm_mode": false,
                "freshness": "unknown"
            }),
            json!({}),
        ),
    };

    let wake_resume_json = wake_resume.map(|(from_eval, woke_at)| {
        json!({
            "from_eval_utc": format_rfc3339_lossy(from_eval),
            "woke_at_utc": format_rfc3339_lossy(woke_at)
        })
    });

    let mut day_context = json!({
        "day_phase": day_phase,
        "phase_started_at_utc": format_rfc3339_lossy(phase_started_at_utc),
        "phase_ends_at_utc": format_rfc3339_lossy(phase_ends_at_utc),
        "asleep": asleep,
        "sleep_onset_utc": sleep_onset_utc.map(format_rfc3339_lossy),
        "wake_resume": wake_resume_json,
        // `blend` is a computed value derived from `phase_started_at_utc`
        // and `PHASE_BLEND_MINUTES`; it is included in the manifest for
        // reviewer convenience.
        "blend": blend
    });

    if let Some(obj) = day_context.as_object_mut() {
        if let Some(extras_obj) = extras.as_object() {
            for (k, v) in extras_obj {
                obj.insert(k.clone(), v.clone());
            }
        }
    }

    BTreeMap::from([
        (
            "fixed_now".to_string(),
            Value::String(format_rfc3339_lossy(fixed_now)),
        ),
        ("tick".to_string(), json!(fixed_now.unix_timestamp())),
        ("terminal_width".to_string(), json!(frame.width)),
        ("terminal_height".to_string(), json!(frame.height)),
        ("day_context".to_string(), day_context),
        ("life_profile".to_string(), life_profile),
    ])
}

// The `activity_profile` values below are fixed scenario-intent tags, not
// computed metrics. They describe the kind of activity identity contract each
// preview frame is meant to exercise.
fn activity_identity_inputs_for_frame(
    id: &str,
    frame: &PreviewFrame,
    ctx: &PreviewRenderContext,
) -> BTreeMap<String, Value> {
    let (source_mix, source_diversity) = match id {
        "watch-activity-identity-ensemble" => (
            json!(["claude-code", "codex", "gemini", "opencode"]),
            "ensemble",
        ),
        "watch-activity-identity-unknown" => (json!(["unknown"]), "single-lane"),
        _ => (json!([]), "quiet"),
    };

    BTreeMap::from([
        (
            "fixed_now".to_string(),
            Value::String(format_rfc3339_lossy(ctx.fixed_now)),
        ),
        ("terminal_width".to_string(), json!(frame.width)),
        ("terminal_height".to_string(), json!(frame.height)),
        ("source_mix".to_string(), source_mix),
        (
            "source_diversity".to_string(),
            Value::String(source_diversity.to_string()),
        ),
        (
            "activity_profile".to_string(),
            json!({
                "source_diversity": source_diversity,
                "rhythm": "bursty",
                "token_shape": "balanced",
                "relative_intensity": "normal",
                "recovery": "sustained",
                "long_term_milestones": []
            }),
        ),
    ])
}

fn species_dialect_inputs_for_frame(id: &str, frame: &PreviewFrame) -> BTreeMap<String, Value> {
    let species = id
        .trim_start_matches("watch-species-dialect-")
        .trim_end_matches("-flat");
    let color_capability = if id.ends_with("-flat") {
        "flat"
    } else {
        "truecolor"
    };
    let dialect_status = frame
        .extra_inputs
        .get("species_dialect")
        .and_then(|v| v.get("status"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| match species {
            "glitch" | "crystal" => "tuned".to_string(),
            _ => "default".to_string(),
        });
    BTreeMap::from([
        ("species".to_string(), Value::String(species.to_string())),
        (
            "room_dialect".to_string(),
            Value::String(species.to_string()),
        ),
        (
            "dialect_status".to_string(),
            Value::String(dialect_status.to_string()),
        ),
        (
            "comparison_group".to_string(),
            Value::String(if matches!(species, "glitch" | "crystal") {
                "species-dialect-glitch-crystal".to_string()
            } else {
                "species-dialect-matrix".to_string()
            }),
        ),
        ("stage".to_string(), Value::String("s6".to_string())),
        ("day_phase".to_string(), Value::String("day".to_string())),
        (
            "work_weather".to_string(),
            Value::String("output-sparks".to_string()),
        ),
        (
            "color_capability".to_string(),
            Value::String(color_capability.to_string()),
        ),
        ("terminal_width".to_string(), json!(frame.width)),
        ("terminal_height".to_string(), json!(frame.height)),
        (
            "earned_prop_ids".to_string(),
            json!([
                "codex_signal_lamp",
                "token_shard_1m",
                "token_orbit_5m",
                "token_lantern_10m"
            ]),
        ),
        (
            "shared_input_invariants".to_string(),
            json!({
                "stage": "s6",
                "day_phase": "day",
                "work_weather": "output-sparks",
                "terminal": [120, 32],
                "earned_prop_ids": ["codex_signal_lamp", "token_shard_1m", "token_orbit_5m", "token_lantern_10m"]
            }),
        ),
        (
            "expected_changed_zones".to_string(),
            json!(["floor-or-anchor", "upper-air-or-pet-adjacent"]),
        ),
        (
            "prop_identity_invariants".to_string(),
            json!([
                "prop ids stay stable",
                "target ids stay stable",
                "catalog colors stay stable",
                "base prop object class stays recognizable"
            ]),
        ),
    ])
}

fn text_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.txt", frame.id))
}

fn cells_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.cells.json", frame.id))
}

fn layout_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.layout.json", frame.id))
}

fn scene_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.scene.json", frame.id))
}

fn round_layout_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.round-layout.json", frame.id))
}

fn round_commands_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.round-commands.json", frame.id))
}

fn room_text_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.room.txt", frame.id))
}

fn room_masked_text_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.room-masked.txt", frame.id))
}

fn preview_mask_rect(target: &crate::tui::component::PreviewTarget) -> PreviewMaskRect {
    PreviewMaskRect {
        x: target.x,
        y: target.y,
        width: target.width,
        height: target.height,
    }
}

fn species_dialect_pair_id(id: &str) -> Option<&'static str> {
    if !has_masked_room_artifact(id) {
        return None;
    }
    if id.ends_with("-flat") {
        Some("flat")
    } else {
        Some("truecolor")
    }
}

fn masked_room_masks_for_species_dialect_pairs(
    frames: &[PreviewFrame],
) -> BTreeMap<String, Vec<PreviewMaskRect>> {
    let mut grouped: BTreeMap<&'static str, Vec<&PreviewFrame>> = BTreeMap::new();
    for frame in frames {
        if let Some(pair_id) = species_dialect_pair_id(&frame.id) {
            grouped.entry(pair_id).or_default().push(frame);
        }
    }

    let mut masks_by_frame = BTreeMap::new();
    for pair_frames in grouped.values() {
        // The paired fixtures use identical earned props and layouts, so
        // unioning prop target rects from both frames yields a stable mask
        // that applies to both.
        let mut prop_masks = Vec::new();
        for frame in pair_frames {
            let Some(layout) = &frame.layout else {
                continue;
            };
            for (target_id, target) in &layout.targets {
                if target_id.starts_with("watch.prop.") {
                    prop_masks.push(preview_mask_rect(target));
                }
            }
        }
        prop_masks.sort_by_key(|rect| (rect.y, rect.x, rect.width, rect.height));
        prop_masks.dedup();

        for frame in pair_frames {
            let layout = frame
                .layout
                .as_ref()
                .expect("strict species dialect frames should have layout");
            let mut masks = Vec::new();
            if let Some(target) = layout.targets.get("watch.pet.art") {
                masks.push(preview_mask_rect(target));
            }
            if let Some(target) = layout.targets.get("watch.pet.speech") {
                masks.push(preview_mask_rect(target));
            }
            masks.extend(prop_masks.iter().copied());
            masks_by_frame.insert(frame.id.clone(), masks);
        }
    }
    masks_by_frame
}

fn color_capability_name(capability: ColorCapability) -> &'static str {
    match capability {
        ColorCapability::Truecolor => "truecolor",
        ColorCapability::Flat => "flat",
    }
}

fn stage_inputs() -> Vec<BTreeMap<&'static str, Value>> {
    [
        Stage::S0,
        Stage::S1,
        Stage::S2,
        Stage::S3,
        Stage::S4,
        Stage::S5,
        Stage::S6,
    ]
    .into_iter()
    .map(|stage| {
        BTreeMap::from([
            ("id", Value::String(stage_id(stage).to_string())),
            (
                "labels_by_species",
                json!(Species::all()
                    .into_iter()
                    .map(|species| (species.as_str(), stage_label(species, stage)))
                    .collect::<BTreeMap<_, _>>()),
            ),
        ])
    })
    .collect()
}

fn stage_id(stage: Stage) -> &'static str {
    match stage {
        Stage::S0 => "s0",
        Stage::S1 => "s1",
        Stage::S2 => "s2",
        Stage::S3 => "s3",
        Stage::S4 => "s4",
        Stage::S5 => "s5",
        Stage::S6 => "s6",
    }
}

fn format_rfc3339(timestamp: OffsetDateTime) -> Result<String> {
    timestamp.format(&Rfc3339).map_err(|err| {
        GlorpError::Message(format!(
            "failed to format preview timestamp {timestamp:?}: {err}"
        ))
    })
}

fn format_rfc3339_lossy(timestamp: OffsetDateTime) -> String {
    format_rfc3339(timestamp).unwrap_or_else(|_| timestamp.unix_timestamp().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_selection_writes_watch_and_pet_scenarios() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("preview");

        generate_preview_bundle(&out, PreviewSelection::All).unwrap();

        let manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(out.join("manifest.json")).unwrap()).unwrap();
        let ids = manifest["scenarios"]
            .as_array()
            .unwrap()
            .iter()
            .map(|scenario| scenario["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec![
                "watch-wide-normal",
                "watch-tall-wide",
                "watch-compact-normal",
                "watch-liveliness-s6-idle-dawn",
                "watch-liveliness-s6-warm-midday",
                "watch-liveliness-s6-hot-midday",
                "watch-liveliness-s6-cooling-evening",
                "watch-liveliness-compact-s6-hot",
                "watch-liveliness-flat-s6-hot",
                "watch-liveliness-calm-mode-s6-hot",
                "watch-daycontext-night-asleep",
                "watch-daycontext-dawn-crossing",
                "watch-daycontext-night-wake-catchup",
                "watch-daycontext-hatch-at-night",
                "watch-daycontext-dream-night",
                "watch-daycontext-heavy-day-evening",
                "watch-daycontext-light-day-morning",
                "watch-daycontext-weekend-midday",
                "watch-daycontext-climate-cache-week",
                "watch-daycontext-prop-resonance-planter",
                "watch-daycontext-midnight-mid-session",
                "watch-daycontext-dawn-fresh",
                "watch-daycontext-dusk-heavy",
                "watch-daycontext-night-quiet",
                "watch-daycontext-work-output-sparks",
                "watch-daycontext-work-reasoning-pulse",
                "watch-daycontext-work-cache-mist",
                "watch-daycontext-work-mixed",
                "watch-daycontext-work-clear",
                "room-starter-day-clear",
                "room-botanical-cache-evening",
                "room-technical-output-active",
                "room-celestial-artifact-night",
                "room-cozy-weekend-quiet",
                "room-mixed-full-wide",
                "room-heavy-day-cozy-large",
                "room-dawn-wake-small",
                "watch-species-dialect-fuzz",
                "watch-species-dialect-blob",
                "watch-species-dialect-ghost",
                "watch-species-dialect-glitch",
                "watch-species-dialect-crystal",
                "watch-species-dialect-mech",
                "watch-species-dialect-glitch-flat",
                "watch-species-dialect-crystal-flat",
                "watch-activity-identity-ensemble",
                "watch-activity-identity-unknown",
                "habitat-props-catalog",
                "watch-habitat-early",
                "watch-habitat-lived-in",
                "watch-habitat-full-phase-a",
                "watch-habitat-full-phase-b",
                "pet-species-stage",
                "pet-species-stage-flat",
                "pet-glitch-live-states",
                "round-normal",
                "round-active-pulse",
                "round-asleep-night",
                "round-helper-trouble",
                "round-flat-color",
                "round-glitch-dialect",
                "round-crystal-dialect"
            ]
        );
    }

    #[test]
    fn preview_render_context_uses_fixed_watch_clock() {
        let ctx = PreviewRenderContext::deterministic();

        assert_eq!(ctx.render.clock.now_utc(), ctx.fixed_now);
    }

    #[test]
    fn scenario_metadata_includes_review_inputs_and_files() {
        let ctx = PreviewRenderContext::deterministic();
        let frame = PreviewFrame {
            id: "watch-wide-normal".to_string(),
            title: "Watch Wide Normal".to_string(),
            width: 120,
            height: 32,
            cells: Vec::new(),
            layout: None,
            extra_inputs: BTreeMap::new(),
            contract: crate::dev_preview::contract::PreviewFrameContract::default(),
        };

        let scenario = scenario_metadata(&frame, &ctx);

        assert_eq!(scenario.kind, PreviewScenarioKind::Watch);
        assert_eq!(
            scenario.files.text,
            PathBuf::from("frames/watch-wide-normal.txt")
        );
        assert_eq!(
            scenario.files.cells,
            PathBuf::from("frames/watch-wide-normal.cells.json")
        );
        assert_eq!(scenario.files.layout, None);
        assert_eq!(scenario.inputs["color_capability"], "truecolor");
        assert_eq!(scenario.inputs["species"], "fuzz");
        assert_eq!(scenario.inputs["stage"], "s4");
        assert!(!scenario.review_prompts.is_empty());
    }

    #[test]
    fn scenario_bundle_pairs_frame_with_manifest_metadata() {
        let ctx = PreviewRenderContext::deterministic();
        let frame = PreviewFrame {
            id: "watch-wide-normal".to_string(),
            title: "Watch Wide Normal".to_string(),
            width: 120,
            height: 32,
            cells: Vec::new(),
            layout: None,
            extra_inputs: BTreeMap::new(),
            contract: crate::dev_preview::contract::PreviewFrameContract::default(),
        };

        let bundle = PreviewScenarioBundle::from_frame(frame, &ctx);

        assert_eq!(bundle.frame.id, "watch-wide-normal");
        assert_eq!(bundle.scenario.id, "watch-wide-normal");
        assert_eq!(
            bundle.scenario.files.text,
            PathBuf::from("frames/watch-wide-normal.txt")
        );
    }

    #[test]
    fn pet_matrix_metadata_lists_species_and_stages() {
        let ctx = PreviewRenderContext::deterministic();
        let frame = PreviewFrame {
            id: "pet-species-stage".to_string(),
            title: "Pet Species Stage".to_string(),
            width: 120,
            height: 86,
            cells: Vec::new(),
            layout: None,
            extra_inputs: BTreeMap::new(),
            contract: crate::dev_preview::contract::PreviewFrameContract::default(),
        };

        let scenario = scenario_metadata(&frame, &ctx);

        assert_eq!(scenario.inputs["mood"], "content");
        assert_eq!(scenario.inputs["tick"], 0);
        assert_eq!(
            scenario.inputs["species"],
            json!(["fuzz", "blob", "ghost", "glitch", "crystal", "mech"])
        );
        assert_eq!(
            scenario.inputs["stages"][0]["labels_by_species"]["fuzz"],
            "fluff"
        );
        assert_eq!(
            scenario.inputs["stages"][6]["labels_by_species"]["mech"],
            "titan"
        );
    }
}
