use super::{
    AmbientFrameSnapshot, AmbientSemanticSnapshot, AuthoredDepthSnapshot, CompanionDayPhase,
    CompanionSceneProjectionError, CompanionSceneProjectionInput, CompanionSceneSnapshot,
    ContentSnapshot, DepthCue, FrameSnapshot, GaugeLevelSnapshot, PaletteSnapshot,
    PetLatticeSnapshot, PetRoleSpanSnapshot, PetTopologySnapshot, PropAnimationKindSnapshot,
    PropAnimationSnapshot, PropTopologySnapshot, PropZoneSnapshot, RoomTopologySnapshot,
    TankAnimationSnapshot, TankBoundsSnapshot, TankCellSnapshot, TankLayerSnapshot,
    TankRouteSnapshot, TankSideSnapshot, TankTopologySnapshot, TopologySnapshot,
    COMPANION_RENDERER_SCHEMA_VERSION, COMPANION_SCENE_SCHEMA_VERSION, MAX_VISIBLE_PROPS,
    MAX_VISIBLE_TANK_INHABITANTS, PET_LATTICE_HEIGHT, PET_LATTICE_SLOTS, PET_LATTICE_WIDTH,
};
use crate::game::habitat::{HabitatPetLayer, HabitatPropZone, TankLifeRouteFamily};
use crate::pet::palette::{body_glow, ResolvedPalette, Rgb};
use crate::pet::render::{PaletteRoleName, StyledSegment};
use crate::presentation::gauge_values::{
    companion_pace_fraction, daily_fraction_for_gauge, daily_overage_marker_fraction,
};
use crate::presentation::privacy::{PresentationSurface, PrivacyProjection};
use crate::tui::day::DayPhase;
use crate::tui::room::{derive_room_life_profile, RoomBiomeTag, RoomLifeProfile, RoomWeatherLayer};
use crate::tui::view_model::{SourceStatus, WatchViewModel};
use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticVitalBucket {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticHelperHealth {
    Ok,
    Trouble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SemanticActivityPulse {
    Quiet,
    Recent { age_ms: u16 },
}

impl SemanticActivityPulse {
    pub(crate) const fn age_ms(self) -> Option<u16> {
        match self {
            Self::Quiet => None,
            Self::Recent { age_ms } => Some(age_ms),
        }
    }

    pub(crate) const fn is_quiet(self) -> bool {
        matches!(self, Self::Quiet)
    }

    pub(crate) fn opacity(self) -> f32 {
        self.age_ms()
            .map(|age| 1.0 - f32::from(age).min(2_000.0) / 2_000.0)
            .unwrap_or(0.0)
    }
}

pub(crate) fn derive_vital_bucket(value: f64) -> SemanticVitalBucket {
    if value < 0.34 {
        SemanticVitalBucket::Low
    } else if value < 0.67 {
        SemanticVitalBucket::Medium
    } else {
        SemanticVitalBucket::High
    }
}

pub(crate) fn derive_helper_health(vm: &WatchViewModel) -> SemanticHelperHealth {
    if vm
        .source_health
        .iter()
        .any(|health| health.status == SourceStatus::Diagnostic)
    {
        SemanticHelperHealth::Trouble
    } else {
        SemanticHelperHealth::Ok
    }
}

pub(crate) fn derive_activity_pulse(
    vm: &WatchViewModel,
    now: OffsetDateTime,
) -> SemanticActivityPulse {
    if vm.day_context.asleep {
        return SemanticActivityPulse::Quiet;
    }
    let Some(last) = vm.last_feed_pulse_at else {
        return SemanticActivityPulse::Quiet;
    };
    let age = now - last;
    if age < Duration::ZERO || age > Duration::seconds(2) {
        return SemanticActivityPulse::Quiet;
    }
    SemanticActivityPulse::Recent {
        age_ms: age.whole_milliseconds().clamp(0, i128::from(u16::MAX)) as u16,
    }
}

pub(crate) fn derive_room_profile(vm: &WatchViewModel, now: OffsetDateTime) -> RoomLifeProfile {
    derive_room_life_profile(vm, now)
}

fn derive_visible_prop_catalog_ids(vm: &WatchViewModel, now: OffsetDateTime) -> Vec<&'static str> {
    let props = vm
        .habitat
        .earned_props
        .iter()
        .map(
            |prop| crate::presentation::habitat_inventory::HabitatPropRecord {
                id: prop.id.as_str(),
                earned_at: prop.earned_at,
                kind: prop.kind,
                display_priority: prop.display_priority,
            },
        )
        .collect::<Vec<_>>();
    let trophy_ids = crate::presentation::habitat_inventory::visible_trophy_ids(&props);
    let accent_ids = crate::presentation::habitat_inventory::visible_accent_ids(&props, now);

    trophy_ids
        .into_iter()
        .chain(accent_ids)
        .filter_map(|id| crate::game::habitat::catalog_prop_by_str(id).map(|spec| spec.id))
        .take(MAX_VISIBLE_PROPS)
        .collect()
}

fn derive_visible_round_tank_catalog_ids(vm: &WatchViewModel) -> Vec<&'static str> {
    let unlocked = vm
        .habitat
        .earned_inhabitants
        .iter()
        .map(|earned| earned.id.clone())
        .collect::<Vec<_>>();
    crate::presentation::habitat_inventory::canonical_daily_cast(
        &unlocked,
        &vm.pet_render.seed,
        vm.habitat.tank_life_local_date,
        vm.habitat.tank_life_calendar_age_days,
    )
    .iter()
    .filter_map(crate::game::habitat::tank_inhabitant_spec)
    .map(|spec| spec.id)
    .take(MAX_VISIBLE_TANK_INHABITANTS)
    .collect()
}

impl CompanionSceneSnapshot {
    pub fn project_with_input(
        vm: &WatchViewModel,
        input: CompanionSceneProjectionInput,
    ) -> Result<Self, CompanionSceneProjectionError> {
        if input.grid_columns == 0 || input.grid_rows == 0 {
            return Err(CompanionSceneProjectionError::InvalidProjectionGrid);
        }
        if !input.layout.width_points.is_finite()
            || !input.layout.height_points.is_finite()
            || input.layout.width_points <= 0.0
            || input.layout.height_points <= 0.0
        {
            return Err(CompanionSceneProjectionError::InvalidProjectionLayout);
        }
        let cell_extent_points = [
            input.layout.width_points / f32::from(input.grid_columns),
            input.layout.height_points / f32::from(input.grid_rows),
        ];
        if cell_extent_points
            .iter()
            .any(|extent| !extent.is_finite() || *extent <= 0.0)
        {
            return Err(CompanionSceneProjectionError::InvalidProjectionLayout);
        }
        let now = input.clock.wall_time;
        let layout = input.layout;
        let room_profile = derive_room_profile(vm, now);
        let activity_pulse = derive_activity_pulse(vm, now);
        let helper_health = derive_helper_health(vm);
        let pet_lines = normalize_pet_lattice(vm)?;
        let pet_roles = project_pet_roles(&vm.pet_art, &vm.pet_spans)?;
        let elapsed_ms = input.clock.elapsed_ms;
        let roam_motion = crate::round::motion::companion_roam_motion();
        let motion_input = companion_motion_input(vm, now, &roam_motion);
        let motion = crate::round::motion::project_round_companion_motion_with_options(
            motion_input,
            now,
            elapsed_ms,
            input.motion_viewport(),
            &roam_motion,
            crate::round::motion::RoundMotionProjectionOptions {
                depth_override: input.depth_override,
            },
        );
        let visible_props = project_props(vm, now);
        let visible_tank_inhabitants = project_tank_inhabitants(vm);
        let prop_animation_states = project_prop_animation_states(vm, &visible_props, now, layout);
        let tank_animation_states =
            project_tank_animation_states(vm, &visible_tank_inhabitants, input, motion)?;
        let (ambient_semantics, ambient_instances) = project_ambient_slots();
        let (room_glyphs, room_glyph_frames) =
            project_room_glyphs(&room_profile, vm, motion, input, cell_extent_points)?;
        let asleep = vm.day_context.asleep;
        let calm = vm.life_profile.calm_mode || asleep;
        let dimmed = asleep || calm;
        let gauge_fractions = project_gauge_fractions(vm);
        let depth = crate::round::depth::resolve_smooth_depth(
            motion.normalized_depth,
            crate::round::depth::depth_lifecycle_scale(asleep, calm),
        )
        .map_err(|_| CompanionSceneProjectionError::InvalidDepthProjection)?;

        Ok(Self {
            schema_version: COMPANION_SCENE_SCHEMA_VERSION,
            privacy: PrivacyProjection::for_surface(PresentationSurface::RoundCompanion),
            topology: TopologySnapshot {
                layout,
                glyph_grid: super::CompanionGlyphGrid {
                    columns: input.grid_columns,
                    rows: input.grid_rows,
                    y_up_origin_points: [0.0, 0.0],
                    cell_extent_points,
                    scale: super::LogicalGlyphScale::OneCell,
                    anchor: super::LogicalGlyphAnchor::CellBottomLeft,
                },
                pet: PetTopologySnapshot {
                    species: vm.pet_render.generated_species,
                    stage: vm.pet_render.stage,
                    lattice: PetLatticeSnapshot {
                        identity: "pet-art-13x10-v1",
                        width: PET_LATTICE_WIDTH,
                        height: PET_LATTICE_HEIGHT,
                        slot_count: PET_LATTICE_SLOTS,
                    },
                },
                room: project_room(&room_profile),
                visible_props,
                visible_tank_inhabitants,
                renderer_schema: COMPANION_RENDERER_SCHEMA_VERSION,
            },
            content: ContentSnapshot {
                mood: vm.pet_render.mood,
                room_weather: room_weather_alias(room_profile.room_weather),
                day_phase: companion_day_phase(vm.day_context.day_phase),
                pet_lines,
                pet_roles,
                room_glyphs,
                palette: PaletteSnapshot::from(vm.pet_palette),
                prop_animation_states,
                tank_animation_states,
                ambient_semantics,
            },
            frame: FrameSnapshot {
                elapsed_ms,
                pet_anchor_points: motion.motion_top_left_points,
                pet_depth: motion.normalized_depth,
                pet_depth_cue: DepthCue {
                    scale: depth.scale,
                    y_offset_points_up: -depth.perspective_y * cell_extent_points[1],
                    opacity: depth.atmosphere,
                    saturation: 1.0,
                },
                facing: motion.facing,
                breath_offset_y_points: f32::from(motion.breath_offset_y_cells)
                    * (layout.height_points / f32::from(input.grid_rows.max(1))),
                bob_offset_y_points: motion.bob_offset_y_cells
                    * (layout.height_points / f32::from(input.grid_rows.max(1))),
                asleep,
                calm,
                helper_trouble: helper_health == SemanticHelperHealth::Trouble,
                activity_recent: !activity_pulse.is_quiet(),
                activity_opacity: activity_pulse.opacity(),
                gauge_levels: gauge_fractions
                    .map(|value| GaugeLevelSnapshot::from_fraction(f64::from(value))),
                gauge_fractions,
                dimmed,
                dim_amount: if dimmed { 0.35 } else { 0.0 },
                room_glyphs: room_glyph_frames,
                ambient_instances,
            },
        })
    }
}

fn project_gauge_fractions(vm: &WatchViewModel) -> [f32; 4] {
    [
        if vm.progress.is_max_stage {
            1.0
        } else {
            vm.progress.fraction
        },
        daily_fraction_for_gauge(vm.daily_comparison.fraction_of_yesterday) as f32,
        daily_overage_marker_fraction(vm.daily_comparison.fraction_of_yesterday) as f32,
        companion_pace_fraction(vm.rate_momentum.pulse.current_tokens) as f32,
    ]
    .map(|value| {
        if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        }
    })
}

const fn companion_day_phase(phase: DayPhase) -> CompanionDayPhase {
    match phase {
        DayPhase::Dawn => CompanionDayPhase::Dawn,
        DayPhase::Day => CompanionDayPhase::Day,
        DayPhase::Dusk => CompanionDayPhase::Dusk,
        DayPhase::Night => CompanionDayPhase::Night,
    }
}

pub(super) fn companion_motion_input(
    vm: &WatchViewModel,
    now: OffsetDateTime,
    motion: &crate::round::motion::CompanionMotion,
) -> crate::round::motion::CompanionMotionInput {
    let wander_width = PET_LATTICE_WIDTH + 2 * motion.wander_half;
    let (resolved_wander_offset_x, resolved_wander_facing) =
        crate::tui::wander::resolve_wander_offset(vm, now, wander_width);
    crate::round::motion::CompanionMotionInput {
        asleep: vm.day_context.asleep,
        calm: vm.life_profile.calm_mode,
        rate_per_hour: vm.progress.rate_per_hour,
        current_facing: vm.facing,
        resolved_wander_offset_x,
        resolved_wander_facing,
        breath_offset_y_cells: vm.breath_offset_y,
    }
}

fn project_room(profile: &RoomLifeProfile) -> RoomTopologySnapshot {
    RoomTopologySnapshot {
        primary_biome: biome_alias(profile.biome.primary),
        secondary_biome: profile.biome.secondary.map(biome_alias),
        species_dialect: profile.species_dialect.key.as_str(),
    }
}

fn project_props(vm: &WatchViewModel, now: OffsetDateTime) -> Vec<PropTopologySnapshot> {
    derive_visible_prop_catalog_ids(vm, now)
        .into_iter()
        .enumerate()
        .filter_map(|(index, id)| {
            let spec = crate::game::habitat::catalog_prop_by_str(id)?;
            Some(PropTopologySnapshot {
                catalog_id: spec.id,
                stable_order: index as u8,
                zone: spec.zone.into(),
                authored_depth: spec.pet_layer.into(),
            })
        })
        .collect()
}

fn project_tank_inhabitants(vm: &WatchViewModel) -> Vec<TankTopologySnapshot> {
    derive_visible_round_tank_catalog_ids(vm)
        .into_iter()
        .enumerate()
        .filter_map(|(index, id)| {
            let spec = crate::game::habitat::TANK_INHABITANT_CATALOG
                .iter()
                .find(|spec| spec.id == id)?;
            Some(TankTopologySnapshot {
                catalog_id: spec.id,
                stable_order: index as u8,
                route: spec.route_family.into(),
                authored_depth: spec.natural_layer.into(),
            })
        })
        .collect()
}

fn normalize_pet_lattice(
    vm: &WatchViewModel,
) -> Result<Vec<String>, CompanionSceneProjectionError> {
    if vm.pet_art.len() > usize::from(PET_LATTICE_HEIGHT) {
        return Err(CompanionSceneProjectionError::PetArtTooTall { row_count: vm.pet_art.len() });
    }

    let declared = crate::pet::render::declared_pet_glyphs(vm.pet_render.generated_species);
    let mut rows = Vec::with_capacity(usize::from(PET_LATTICE_HEIGHT));
    for (line_index, source) in vm.pet_art.iter().enumerate() {
        let chars = source.chars().collect::<Vec<_>>();
        if chars.len() > usize::from(PET_LATTICE_WIDTH) {
            return Err(CompanionSceneProjectionError::PetArtTooWide {
                line_index,
                char_count: chars.len(),
            });
        }
        for (char_index, glyph) in chars.iter().copied().enumerate() {
            if glyph != ' ' && !declared.contains(&glyph) {
                return Err(CompanionSceneProjectionError::DisallowedPetGlyph {
                    line_index,
                    char_index,
                });
            }
        }
        let mut row = chars;
        row.resize(usize::from(PET_LATTICE_WIDTH), ' ');
        rows.push(row.into_iter().collect());
    }
    rows.resize(
        usize::from(PET_LATTICE_HEIGHT),
        " ".repeat(usize::from(PET_LATTICE_WIDTH)),
    );
    Ok(rows)
}

fn project_pet_roles(
    source_lines: &[String],
    spans: &[StyledSegment],
) -> Result<Vec<PetRoleSpanSnapshot>, CompanionSceneProjectionError> {
    spans
        .iter()
        .enumerate()
        .map(|(span_index, span)| {
            let source_char_count = source_lines
                .get(span.line)
                .map_or(0, |line| line.chars().count());
            if span.line >= usize::from(PET_LATTICE_HEIGHT)
                || span.line >= source_lines.len()
                || span.start >= span.end
                || span.end > source_char_count
            {
                return Err(CompanionSceneProjectionError::InvalidPetRoleSpan {
                    span_index,
                    line_index: span.line,
                    start_char: span.start,
                    end_char: span.end,
                    source_char_count,
                });
            }
            Ok(PetRoleSpanSnapshot {
                line_index: span.line as u16,
                start_char: span.start as u16,
                end_char: span.end as u16,
                role: role_alias(span.role),
            })
        })
        .collect()
}

fn project_prop_animation_states(
    vm: &WatchViewModel,
    visible_props: &[PropTopologySnapshot],
    now: OffsetDateTime,
    layout: super::CompanionLogicalLayout,
) -> Vec<PropAnimationSnapshot> {
    visible_props
        .iter()
        .map(|prop| {
            let state = crate::game::habitat::habitat_prop_animation_state(prop.catalog_id, now);
            PropAnimationSnapshot {
                catalog_id: prop.catalog_id,
                stable_order: prop.stable_order,
                kind: if state.is_static() {
                    PropAnimationKindSnapshot::Static
                } else {
                    PropAnimationKindSnapshot::Animated
                },
                sprite_phase: state.sprite_phase,
                twinkle_active: state.twinkle_active,
                motion_phase: state.motion_phase,
                chest_lid_open: state.chest_lid_open,
                bloom_active: is_blooming_prop(prop.catalog_id).then(|| {
                    vm.habitat
                        .earned_props
                        .iter()
                        .find(|earned| earned.id.as_str() == prop.catalog_id)
                        .is_some_and(|earned| (now - earned.earned_at).whole_days() >= 3)
                }),
                origin_points: resolved_prop_origin(prop.zone, prop.stable_order, layout),
            }
        })
        .collect()
}

fn is_blooming_prop(catalog_id: &str) -> bool {
    crate::game::habitat::habitat_prop_supports_bloom(catalog_id)
}

fn project_tank_animation_states(
    vm: &WatchViewModel,
    visible_tank_inhabitants: &[TankTopologySnapshot],
    input: CompanionSceneProjectionInput,
    motion: crate::round::motion::RoundCompanionMotionProjection,
) -> Result<Vec<TankAnimationSnapshot>, CompanionSceneProjectionError> {
    let calm = vm.life_profile.calm_mode || vm.day_context.asleep;
    let mut geometry = crate::presentation::tank_life::TankRouteGeometry::round(
        input.grid_columns,
        input.grid_rows,
        input.motion_clearance.bottom_reserved_rows,
    );
    geometry.foreground_reserved_regions.push(
        crate::presentation::tank_life::pet_face_reserved_region(
            crate::presentation::tank_life::TankRouteRect {
                x: motion.classic_top_left_cells[0],
                y: motion.classic_top_left_cells[1],
                width: PET_LATTICE_WIDTH,
                height: PET_LATTICE_HEIGHT,
            },
        ),
    );
    visible_tank_inhabitants
        .iter()
        .map(|inhabitant| {
            let paint = crate::presentation::tank_life::tank_paint_for(inhabitant.catalog_id)
                .ok_or(CompanionSceneProjectionError::InvalidTankPaint)?;
            let outcome = crate::presentation::tank_life::resolve_tank_route(
                crate::presentation::tank_life::TankRouteInput {
                    catalog_id: inhabitant.catalog_id,
                    pet_seed: &vm.pet_render.seed,
                    local_date: vm.habitat.tank_life_local_date,
                    now: input.clock.wall_time,
                    calm,
                    geometry: &geometry,
                },
            )
            .ok_or(CompanionSceneProjectionError::InvalidTankRoute)?;
            Ok(TankAnimationSnapshot {
                catalog_id: inhabitant.catalog_id,
                stable_order: inhabitant.stable_order,
                route: outcome.route.into(),
                visible: outcome.visible,
                origin_col: outcome.origin_col,
                origin_row: outcome.origin_row,
                origin_points: grid_cell_to_points(outcome.origin_col, outcome.origin_row, input),
                side: outcome.side.map(|side| match side {
                    crate::presentation::tank_life::TankRouteSide::Left => TankSideSnapshot::Left,
                    crate::presentation::tank_life::TankRouteSide::Right => TankSideSnapshot::Right,
                    crate::presentation::tank_life::TankRouteSide::Rear => TankSideSnapshot::Rear,
                    crate::presentation::tank_life::TankRouteSide::Front => TankSideSnapshot::Front,
                }),
                layer: match outcome.layer {
                    crate::presentation::tank_life::TankRouteLayer::Behind => {
                        TankLayerSnapshot::Behind
                    }
                    crate::presentation::tank_life::TankRouteLayer::Foreground => {
                        TankLayerSnapshot::Foreground
                    }
                    crate::presentation::tank_life::TankRouteLayer::BehindAnchorForegroundHost => {
                        TankLayerSnapshot::BehindAnchorForegroundHost
                    }
                },
                sprite_variant: outcome.sprite_variant,
                visible_rows: outcome.visible_rows,
                anemone_morph: outcome.anemone_morph,
                color_srgb8: paint.color_srgb8,
                bold: paint.bold,
                cadence_ms: outcome.cadence_ms,
                calm: outcome.calm,
                cells: outcome
                    .cells
                    .iter()
                    .map(|cell| TankCellSnapshot {
                        col: cell.col,
                        row: cell.row,
                        glyph: cell.glyph,
                        layer: match cell.layer {
                            crate::presentation::tank_life::TankRouteLayer::Behind => {
                                TankLayerSnapshot::Behind
                            }
                            crate::presentation::tank_life::TankRouteLayer::Foreground => {
                                TankLayerSnapshot::Foreground
                            }
                            crate::presentation::tank_life::TankRouteLayer::BehindAnchorForegroundHost => {
                                unreachable!("combined route layer is not a resolved cell layer")
                            }
                        },
                        position_points: grid_cell_to_points(
                            cell.col,
                            cell.row,
                            input,
                        ),
                    })
                    .collect(),
                bounds: outcome.bounds.map(|bounds| TankBoundsSnapshot {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width,
                    height: bounds.height,
                }),
                bounds_points: outcome.bounds.map(|bounds| {
                    let origin = grid_cell_to_points(bounds.x, bounds.y, input);
                    let cell_width = input.layout.width_points / f32::from(input.grid_columns.max(1));
                    let cell_height = input.layout.height_points / f32::from(input.grid_rows.max(1));
                    [
                        origin[0],
                        origin[1],
                        f32::from(bounds.width) * cell_width,
                        f32::from(bounds.height) * cell_height,
                    ]
                }),
            })
        })
        .collect()
}

fn resolved_prop_origin(
    zone: PropZoneSnapshot,
    stable_order: u8,
    layout: super::CompanionLogicalLayout,
) -> [f32; 2] {
    let [x, y] = match zone {
        PropZoneSnapshot::FloorLeft => [0.18, 0.78],
        PropZoneSnapshot::FloorMid => [0.47, 0.78],
        PropZoneSnapshot::FloorRight => [0.76, 0.78],
        PropZoneSnapshot::WallLeft => [0.14, 0.48],
        PropZoneSnapshot::WallRight => [0.82, 0.48],
        PropZoneSnapshot::AirLeft => [0.22, 0.28],
        PropZoneSnapshot::AirMid => [0.48, 0.25],
        PropZoneSnapshot::AirRight => [0.74, 0.28],
        PropZoneSnapshot::Ceiling => [0.50, 0.10],
    };
    let lane = f32::from(stable_order % 3) - 1.0;
    [
        (x + lane * 0.025) * layout.width_points,
        y * layout.height_points,
    ]
}

fn grid_cell_to_points(col: u16, row: u16, input: CompanionSceneProjectionInput) -> [f32; 2] {
    let cell_width = input.layout.width_points / f32::from(input.grid_columns.max(1));
    let cell_height = input.layout.height_points / f32::from(input.grid_rows.max(1));
    [
        (f32::from(col) + 0.5) * cell_width,
        (f32::from(row) + 0.5) * cell_height,
    ]
}

fn project_room_glyphs(
    profile: &RoomLifeProfile,
    vm: &WatchViewModel,
    motion: crate::round::motion::RoundCompanionMotionProjection,
    input: CompanionSceneProjectionInput,
    cell_extent_points: [f32; 2],
) -> Result<
    (
        Vec<super::RoomGlyphContentSnapshot>,
        Vec<super::RoomGlyphFrameSnapshot>,
    ),
    CompanionSceneProjectionError,
> {
    let glyphs = crate::tui::room::companion_room_glyphs_for(
        profile,
        crate::tui::room::CompanionRoomProjectionInput {
            pet_art: &vm.pet_art,
            speech_visible: vm.current_speech.is_some(),
            day_phase: vm.day_context.day_phase,
            columns: input.grid_columns,
            rows: input.grid_rows,
            classic_pet_top_left: motion.classic_top_left_cells,
            pet_frame_extent: [super::PET_LATTICE_WIDTH, super::PET_LATTICE_HEIGHT],
            facing: motion.facing,
            now: input.clock.wall_time,
        },
    )
    .map_err(|_| CompanionSceneProjectionError::InvalidRoomGlyphColor)?;
    if glyphs.len() > super::scene::MAX_ROOM_GLYPH_SLOTS {
        return Err(CompanionSceneProjectionError::RoomGlyphCapacity { count: glyphs.len() });
    }
    let mut content = Vec::with_capacity(glyphs.len());
    let mut frame = Vec::with_capacity(glyphs.len());
    for (slot, glyph) in glyphs.into_iter().enumerate() {
        content.push(super::RoomGlyphContentSnapshot {
            slot: slot as u8,
            glyph: glyph.glyph,
            color_srgb8: glyph.color_rgb,
        });
        frame.push(super::RoomGlyphFrameSnapshot {
            slot: slot as u8,
            visible: true,
            grid_cell: [glyph.col, glyph.row],
            position_points: [
                f32::from(glyph.col) * cell_extent_points[0],
                input.layout.height_points - (f32::from(glyph.row) + 1.0) * cell_extent_points[1],
            ],
            opacity: 1.0,
        });
    }
    Ok((content, frame))
}

fn project_ambient_slots() -> (Vec<AmbientSemanticSnapshot>, Vec<AmbientFrameSnapshot>) {
    let semantics = (0..super::scene::MAX_AMBIENT_INSTANCES)
        .map(|slot| AmbientSemanticSnapshot {
            slot: slot as u8,
            kind: None,
            glyph: None,
        })
        .collect::<Vec<_>>();
    let frames = (0..super::scene::MAX_AMBIENT_INSTANCES)
        .map(|slot| AmbientFrameSnapshot {
            slot: slot as u8,
            visible: false,
            position_points: [0.0; 2],
            opacity: 0.0,
        })
        .collect();
    (semantics, frames)
}

fn biome_alias(biome: RoomBiomeTag) -> &'static str {
    match biome {
        RoomBiomeTag::Starter => "starter",
        RoomBiomeTag::Botanical => "botanical",
        RoomBiomeTag::Technical => "technical",
        RoomBiomeTag::Celestial => "celestial",
        RoomBiomeTag::Artifact => "artifact",
        RoomBiomeTag::Cozy => "cozy",
    }
}

fn room_weather_alias(weather: RoomWeatherLayer) -> &'static str {
    match weather {
        RoomWeatherLayer::Clear => "clear",
        RoomWeatherLayer::CacheMist => "cache-mist",
        RoomWeatherLayer::OutputSparks => "output-sparks",
        RoomWeatherLayer::ReasoningPulse => "reasoning-pulse",
        RoomWeatherLayer::Mixed => "mixed",
    }
}

fn role_alias(role: PaletteRoleName) -> &'static str {
    match role {
        PaletteRoleName::Body => "body",
        PaletteRoleName::BodyGlow => "body-glow",
        PaletteRoleName::Eye => "eye",
        PaletteRoleName::Mouth => "mouth",
        PaletteRoleName::Accent => "accent",
        PaletteRoleName::Pattern => "pattern",
        PaletteRoleName::Particle => "particle",
        PaletteRoleName::Corruption => "corruption",
    }
}

fn rgb(rgb: Rgb) -> [u8; 3] {
    [rgb.r, rgb.g, rgb.b]
}

impl From<ResolvedPalette> for PaletteSnapshot {
    fn from(palette: ResolvedPalette) -> Self {
        Self {
            body: rgb(palette.body),
            body_glow: rgb(body_glow(palette.body)),
            eye: rgb(palette.eye),
            mouth: rgb(palette.mouth),
            accent: rgb(palette.accent),
            pattern: rgb(palette.pattern),
            particle: rgb(palette.particle),
            corruption: rgb(palette.corruption),
        }
    }
}

impl From<HabitatPropZone> for PropZoneSnapshot {
    fn from(zone: HabitatPropZone) -> Self {
        match zone {
            HabitatPropZone::FloorLeft => Self::FloorLeft,
            HabitatPropZone::FloorMid => Self::FloorMid,
            HabitatPropZone::FloorRight => Self::FloorRight,
            HabitatPropZone::WallLeft => Self::WallLeft,
            HabitatPropZone::WallRight => Self::WallRight,
            HabitatPropZone::AirLeft => Self::AirLeft,
            HabitatPropZone::AirMid => Self::AirMid,
            HabitatPropZone::AirRight => Self::AirRight,
            HabitatPropZone::Ceiling => Self::Ceiling,
        }
    }
}

impl From<TankLifeRouteFamily> for TankRouteSnapshot {
    fn from(route: TankLifeRouteFamily) -> Self {
        match route {
            TankLifeRouteFamily::CrossTankSwimmer => Self::CrossTankSwimmer,
            TankLifeRouteFamily::LowerLaneResident => Self::LowerLaneResident,
            TankLifeRouteFamily::GlassResident => Self::GlassResident,
            TankLifeRouteFamily::RimResident => Self::RimResident,
            TankLifeRouteFamily::LowerEdgeResident => Self::LowerEdgeResident,
            TankLifeRouteFamily::HostCombo => Self::HostCombo,
        }
    }
}

impl From<HabitatPetLayer> for AuthoredDepthSnapshot {
    fn from(layer: HabitatPetLayer) -> Self {
        match layer {
            HabitatPetLayer::Background => Self::Background,
            HabitatPetLayer::Behind => Self::BehindPet,
            HabitatPetLayer::Foreground => Self::Foreground,
        }
    }
}
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::game::evolution::Stage;
    use crate::game::metabolism::Mood;
    use crate::pet::generation::{generate_pet, Species};
    use crate::pet::render::{render_pet, AnimationFrame, PaletteRoleName, StyledSegment};
    use crate::presentation::companion_scene::{
        CompanionLogicalLayout, CompanionProjectionClock, CompanionSceneProjectionError,
        CompanionSceneProjectionInput, CompanionSceneSnapshot, GaugeLevelSnapshot,
        PropAnimationSnapshot, PET_LATTICE_HEIGHT, PET_LATTICE_WIDTH,
    };
    use crate::tui::view_model::{SourceStatus, WatchViewModel};
    use time::macros::datetime;

    fn fixture_with_real_pet_art() -> WatchViewModel {
        let mut vm = WatchViewModel::fixture_with_habitat_props();
        let rendered = render_pet(
            &generate_pet("companion-scene-real-art").with_species(Species::Fuzz),
            Stage::S3,
            Mood::Content,
            AnimationFrame::default(),
        );
        vm.pet_render.generated_species = Species::Fuzz;
        vm.pet_render.stage = Stage::S3;
        vm.pet_art = rendered.lines;
        vm.pet_spans = rendered.spans;
        vm
    }

    fn lifetime_watch_fixture() -> WatchViewModel {
        let day = time::macros::date!(2026 - 07 - 11);
        let mut vm = WatchViewModel::fixture_with_tank_inhabitants_for_age(120, day);
        vm.habitat.earned_props = WatchViewModel::fixture_with_habitat_props()
            .habitat
            .earned_props;
        let rendered = render_pet(
            &generate_pet("neutral-analytic-production-lifetime").with_species(Species::Fuzz),
            Stage::S3,
            Mood::Content,
            AnimationFrame::default(),
        );
        vm.pet_render.generated_species = Species::Fuzz;
        vm.pet_render.stage = Stage::S3;
        vm.pet_render.mood = Mood::Content;
        vm.pet_art = rendered.lines;
        vm.pet_spans = rendered.spans;
        vm
    }

    fn project_snapshot(
        vm: &WatchViewModel,
        wall_time: time::OffsetDateTime,
        layout: CompanionLogicalLayout,
    ) -> Result<CompanionSceneSnapshot, CompanionSceneProjectionError> {
        CompanionSceneSnapshot::project_with_input(
            vm,
            CompanionSceneProjectionInput::round(
                CompanionProjectionClock::new(wall_time, 0),
                layout,
                44,
                18,
                crate::round::scene::current_round_motion_clearance(18),
            ),
        )
    }

    #[test]
    fn three_hundred_real_projections_close_neutral_analytic_and_paint_tables() {
        use crate::presentation::companion_scene::scene::{
            build_scene_generation, build_scene_generation_owned, AnalyticSemantic,
            PropGlyphContent, MAX_ANALYTIC_PARAMS, MAX_PROP_GLYPHS_PER_SLOT,
        };
        use crate::presentation::companion_scene::{
            AppliedRevisions, DeviceEpoch, FrameRevision, LayoutGeneration, ResourceGeneration,
            SceneGenerationKey, SemanticRevision,
        };

        let fixture = lifetime_watch_fixture();
        let key = SceneGenerationKey {
            device: DeviceEpoch(90),
            layout: LayoutGeneration(91),
            resources: ResourceGeneration(92),
        };
        let snapshot_at = |frame_index: usize| {
            let mut vm = fixture.clone();
            vm.pet_render.mood = if (frame_index / 25).is_multiple_of(2) {
                Mood::Content
            } else {
                Mood::Happy
            };
            vm.day_context.asleep = (frame_index / 40) % 2 == 1;
            vm.life_profile.calm_mode = (frame_index / 30) % 2 == 1;
            vm.progress.fraction = (frame_index % 101) as f32 / 100.0;
            vm.source_health[0].status = if (frame_index / 45).is_multiple_of(2) {
                SourceStatus::Ready
            } else {
                SourceStatus::Diagnostic
            };
            let wall_time = datetime!(2026-07-11 12:00:55 UTC)
                + time::Duration::seconds(i64::try_from(frame_index).unwrap());
            vm.last_feed_pulse_at = (frame_index % 20 < 10).then_some(
                wall_time
                    - time::Duration::milliseconds(i64::try_from(frame_index % 10).unwrap() * 150),
            );
            CompanionSceneSnapshot::project_with_input(
                &vm,
                CompanionSceneProjectionInput::round(
                    CompanionProjectionClock::new(
                        wall_time,
                        u64::try_from(frame_index).unwrap() * 33,
                    ),
                    CompanionLogicalLayout::round(360.0, 360.0),
                    44,
                    18,
                    crate::round::scene::current_round_motion_clearance(18),
                ),
            )
            .unwrap()
        };

        let initial = Arc::new(snapshot_at(0));
        let mut revisions = AppliedRevisions::new(0, 0);
        let mut built = build_scene_generation_owned(Arc::clone(&initial), key, revisions).unwrap();
        let template_checksum = built.template().generation_checksum;
        let capacities = built.delta_capacities();
        let storage_pointers = built.delta_storage_pointers();
        let mut previous = initial;

        for frame_index in 0_usize..300 {
            let newest = if frame_index == 0 {
                Arc::clone(&previous)
            } else {
                Arc::new(snapshot_at(frame_index))
            };
            if frame_index != 0 {
                let changes =
                    crate::presentation::companion_scene::runtime::classify_snapshot_changes(
                        &previous, &newest,
                    );
                assert!(!changes.requires_generation(), "frame {frame_index}");
                let next = AppliedRevisions {
                    semantic: SemanticRevision(
                        revisions.semantic.0
                            + u64::from(
                                changes.semantic()
                                    != crate::presentation::companion_scene::runtime::SemanticChangeMask::NONE,
                            ),
                    ),
                    frame: FrameRevision(
                        revisions.frame.0
                            + u64::from(
                                changes.frame()
                                    != crate::presentation::companion_scene::runtime::FrameChangeMask::NONE,
                            ),
                    ),
                };
                built
                    .apply_compatible_snapshot(Arc::clone(&newest), changes, revisions, next)
                    .unwrap();
                revisions = next;
                previous = Arc::clone(&newest);
            }

            let fresh = build_scene_generation(&newest, key).unwrap();
            assert_eq!(built.template().generation_checksum, template_checksum);
            assert_eq!(built.template(), fresh.template(), "frame {frame_index}");
            assert_eq!(built.content(), fresh.content(), "frame {frame_index}");
            assert_eq!(built.frame(), fresh.frame(), "frame {frame_index}");
            assert_eq!(built.content_checksum(), fresh.content_checksum());
            assert_eq!(built.frame_checksum(), fresh.frame_checksum());
            assert_eq!(built.delta_capacities(), capacities);
            assert_eq!(built.delta_storage_pointers(), storage_pointers);
            assert_eq!(built.content().analytic_slots.len(), MAX_ANALYTIC_PARAMS);
            assert_eq!(built.frame().analytic_slots.len(), MAX_ANALYTIC_PARAMS);
            assert!(
                built.content().analytic_slots[..AnalyticSemantic::ALL.len()]
                    .iter()
                    .all(|slot| slot.value.is_some())
            );
            assert!(built.frame().analytic_slots[..AnalyticSemantic::ALL.len()]
                .iter()
                .all(|slot| slot.value.is_some()));
            for (content, paint) in built
                .content()
                .prop_slots
                .iter()
                .zip(&built.content().prop_paint_slots)
            {
                let glyphs = content.content.map(|content| content.glyphs).unwrap_or(
                    [PropGlyphContent { glyph: None, local_cell: [0; 2] };
                        MAX_PROP_GLYPHS_PER_SLOT],
                );
                assert!(glyphs
                    .into_iter()
                    .zip(paint.paints)
                    .all(|(glyph, paint)| glyph.glyph.is_some() == paint.is_some()));
            }
        }
    }

    fn prop_animation_state<'a>(
        snapshot: &'a CompanionSceneSnapshot,
        catalog_id: &str,
    ) -> &'a PropAnimationSnapshot {
        snapshot
            .content
            .prop_animation_states
            .iter()
            .find(|state| state.catalog_id == catalog_id)
            .expect("visible prop state")
    }

    #[test]
    fn projection_is_one_privacy_aware_snapshot() {
        let vm = fixture_with_real_pet_art();
        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("project canonical generated pet art");

        assert_eq!(snapshot.schema_version, 2);
        assert_eq!(
            snapshot.topology.pet.species,
            vm.pet_render.generated_species
        );
        assert_eq!(snapshot.topology.pet.stage, vm.pet_render.stage);
        assert_eq!(snapshot.topology.pet.lattice.slot_count, 130);
        assert!(snapshot.topology.visible_props.len() <= 10);
        assert!(snapshot.topology.visible_tank_inhabitants.len() <= 2);
        assert!(!snapshot.privacy.source_names_visible);
        assert!(!snapshot.privacy.file_paths_visible);
        assert_eq!(
            snapshot.content.pet_lines.len(),
            usize::from(PET_LATTICE_HEIGHT)
        );
        assert!(snapshot
            .content
            .pet_lines
            .iter()
            .all(|line| line.chars().count() == usize::from(PET_LATTICE_WIDTH)));
    }

    #[test]
    fn room_projection_is_bounded_colored_y_up_and_minute_dynamic() {
        let vm = fixture_with_real_pet_art();
        let layout = CompanionLogicalLayout::round(440.0, 360.0);
        let first = project_snapshot(&vm, datetime!(2026-07-11 12:00 UTC), layout).unwrap();
        let next = project_snapshot(&vm, datetime!(2026-07-11 12:01 UTC), layout).unwrap();

        assert_eq!(first.topology.glyph_grid.columns, 44);
        assert_eq!(first.topology.glyph_grid.rows, 18);
        assert_eq!(first.topology.glyph_grid.y_up_origin_points, [0.0, 0.0]);
        assert_eq!(first.topology.glyph_grid.cell_extent_points, [10.0, 20.0]);
        assert!(first.content.room_glyphs.len() <= super::super::scene::MAX_ROOM_GLYPH_SLOTS);
        assert_eq!(
            first.content.room_glyphs.len(),
            first.frame.room_glyphs.len()
        );
        assert!(!first.content.room_glyphs.is_empty());
        for (index, (content, frame)) in first
            .content
            .room_glyphs
            .iter()
            .zip(&first.frame.room_glyphs)
            .enumerate()
        {
            assert_eq!(usize::from(content.slot), index);
            assert_eq!(content.slot, frame.slot);
            assert_eq!(
                frame.position_points,
                [
                    f32::from(frame.grid_cell[0]) * 10.0,
                    360.0 - (f32::from(frame.grid_cell[1]) + 1.0) * 20.0,
                ]
            );
            assert!(content.color_srgb8.iter().any(|channel| *channel != 0));
        }
        assert!(
            first.content.room_glyphs != next.content.room_glyphs
                || first.frame.room_glyphs != next.frame.room_glyphs,
            "minute reseed must update room slots without changing authored topology"
        );
        assert_eq!(first.topology, next.topology);
    }

    #[test]
    fn projection_rejects_an_empty_grid_before_point_division() {
        let vm = fixture_with_real_pet_art();
        let input = CompanionSceneProjectionInput::round(
            CompanionProjectionClock::new(datetime!(2026-07-11 12:00 UTC), 0),
            CompanionLogicalLayout::round(360.0, 360.0),
            0,
            18,
            crate::round::scene::current_round_motion_clearance(18),
        );
        assert_eq!(
            CompanionSceneSnapshot::project_with_input(&vm, input),
            Err(CompanionSceneProjectionError::InvalidProjectionGrid)
        );

        let invalid_layout = CompanionSceneProjectionInput::round(
            CompanionProjectionClock::new(datetime!(2026-07-11 12:00 UTC), 0),
            CompanionLogicalLayout::round(f32::NAN, 360.0),
            44,
            18,
            crate::round::scene::current_round_motion_clearance(18),
        );
        assert_eq!(
            CompanionSceneSnapshot::project_with_input(&vm, invalid_layout),
            Err(CompanionSceneProjectionError::InvalidProjectionLayout)
        );

        let underflow_layout = CompanionSceneProjectionInput::round(
            CompanionProjectionClock::new(datetime!(2026-07-11 12:00 UTC), 0),
            CompanionLogicalLayout::round(f32::from_bits(1), 360.0),
            44,
            18,
            crate::round::scene::current_round_motion_clearance(18),
        );
        assert_eq!(
            CompanionSceneSnapshot::project_with_input(&vm, underflow_layout),
            Err(CompanionSceneProjectionError::InvalidProjectionLayout)
        );
    }

    #[test]
    fn projection_resolves_fixed_point_space_instance_inputs() {
        let vm = fixture_with_real_pet_art();
        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .unwrap();

        assert_eq!(snapshot.content.ambient_semantics.len(), 64);
        assert_eq!(snapshot.frame.ambient_instances.len(), 64);
        assert!(snapshot
            .content
            .prop_animation_states
            .iter()
            .all(|state| state.origin_points.iter().all(|value| value.is_finite())));
        assert!(snapshot
            .content
            .tank_animation_states
            .iter()
            .flat_map(|state| &state.cells)
            .all(|cell| cell.position_points.iter().all(|value| value.is_finite())));
    }

    #[test]
    fn pet_cell_offsets_are_projected_to_logical_points() {
        let mut vm = fixture_with_real_pet_art();
        vm.breath_offset_y = 1;
        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .unwrap();
        assert_eq!(snapshot.frame.breath_offset_y_points, 20.0);
        assert!(snapshot.frame.bob_offset_y_points.is_finite());
    }

    #[test]
    fn projection_serialization_excludes_every_private_input_sentinel() {
        let mut vm = fixture_with_real_pet_art();
        vm.pet_render.seed = "sentinel-private-seed-7ea431".to_string();
        vm.pet_name = "sentinel-private-pet-name-9b15".to_string();
        vm.source_breakdown[0].name = "sentinel-private-source-name-6d72".to_string();
        vm.source_breakdown[0].display_name = "sentinel-private-display-name-5c20".to_string();
        vm.source_health[0].diagnostic_message =
            Some("sentinel-private-diagnostic-a03f".to_string());
        vm.helper_status = "sentinel-private-helper-status-284c".to_string();
        vm.errors = vec!["sentinel-private-error-b884 /private/path/scene.json".to_string()];
        vm.current_speech = Some("sentinel-private-speech-token-d37a".to_string());
        vm.today_snapshot_reason = Some("sk-sentinel-private-auth-token-e19c".to_string());
        vm.today_effective_tokens = 987_654_321.25;
        vm.pet_art[0] = "private/path".to_string();
        vm.habitat
            .earned_props
            .push(crate::tui::view_model::EarnedHabitatPropView {
                id: crate::storage::state::HabitatPropId::new("sentinel-private-unknown-prop-725f"),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: crate::game::habitat::HabitatPropKind::Accent,
                display_priority: i16::MAX,
                source: crate::storage::state::HabitatPropSource::ProviderFirstUse {
                    provider_surface: "sentinel-private-provider-source-42e6".to_string(),
                },
            });
        vm.habitat
            .earned_inhabitants
            .push(crate::tui::view_model::EarnedTankInhabitantView {
                id: crate::storage::state::TankInhabitantId::new(
                    "sentinel-private-unknown-inhabitant-f792",
                ),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                unlock_age_days: 0,
                kind: crate::game::habitat::TankInhabitantKind::Swimmer,
                source: crate::storage::state::TankInhabitantSource::PetAgeThreshold {
                    threshold_days: 0,
                },
            });

        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        );
        let error_message = snapshot
            .as_ref()
            .expect_err("private pet text must fail closed")
            .to_string();
        assert!(!error_message.contains("private/path"));
        assert_eq!(
            snapshot,
            Err(CompanionSceneProjectionError::DisallowedPetGlyph { line_index: 0, char_index: 0 })
        );

        vm.pet_art = fixture_with_real_pet_art().pet_art;
        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("project sanitized canonical pet art");
        let json = serde_json::to_string(&snapshot).expect("serialize scene snapshot");

        for sentinel in [
            "sentinel-private-seed-7ea431",
            "sentinel-private-pet-name-9b15",
            "sentinel-private-source-name-6d72",
            "sentinel-private-display-name-5c20",
            "sentinel-private-diagnostic-a03f",
            "sentinel-private-helper-status-284c",
            "sentinel-private-error-b884",
            "/private/path/scene.json",
            "sentinel-private-speech-token-d37a",
            "sk-sentinel-private-auth-token-e19c",
            "sentinel-private-unknown-prop-725f",
            "sentinel-private-provider-source-42e6",
            "sentinel-private-unknown-inhabitant-f792",
            "987654321.25",
        ] {
            assert!(
                !json.contains(sentinel),
                "snapshot leaked {sentinel}: {json}"
            );
        }
        assert!(
            !json.contains("\"seed\""),
            "snapshot exposed a seed field: {json}"
        );
    }

    #[test]
    fn live_display_totals_cannot_change_neutral_scene_or_artifacts() {
        let mut low = fixture_with_real_pet_art();
        low.today_effective_tokens = 1_234.0;
        let mut high = low.clone();
        high.today_effective_tokens = 842_000_000.0;

        let low_snapshot = project_snapshot(
            &low,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("low-total neutral projection");
        let high_snapshot = project_snapshot(
            &high,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("high-total neutral projection");

        assert_eq!(
            serde_json::to_string(&low_snapshot).unwrap(),
            serde_json::to_string(&high_snapshot).unwrap()
        );
        assert_eq!(format!("{low_snapshot:?}"), format!("{high_snapshot:?}"));

        let key = super::super::SceneGenerationKey {
            device: super::super::DeviceEpoch(1),
            layout: super::super::LayoutGeneration(1),
            resources: super::super::ResourceGeneration(1),
        };
        let low_generation = super::super::scene::build_scene_generation(&low_snapshot, key)
            .expect("compile low-total scene");
        let high_generation = super::super::scene::build_scene_generation(&high_snapshot, key)
            .expect("compile high-total scene");
        assert_eq!(
            low_generation.content_checksum(),
            high_generation.content_checksum()
        );
        assert_eq!(
            low_generation.frame_checksum(),
            high_generation.frame_checksum()
        );

        let low_artifacts = super::super::contract::SceneArtifacts::try_from_parts(
            low_generation.template(),
            low_generation.content(),
            low_generation.frame(),
        )
        .expect("low-total artifacts");
        let high_artifacts = super::super::contract::SceneArtifacts::try_from_parts(
            high_generation.template(),
            high_generation.content(),
            high_generation.frame(),
        )
        .expect("high-total artifacts");
        assert_eq!(low_artifacts, high_artifacts);
    }

    #[test]
    fn privacy_projection_quantizes_live_gauges_instead_of_serializing_exact_ratios() {
        let mut vm = fixture_with_real_pet_art();
        vm.progress.fraction = 0.432_109;
        vm.daily_comparison.fraction_of_yesterday = Some(0.943_217);
        vm.rate_momentum.pulse.current_tokens = 31_234_567.0;
        let exact_daily =
            super::daily_fraction_for_gauge(vm.daily_comparison.fraction_of_yesterday) as f32;
        let exact_pace =
            super::companion_pace_fraction(vm.rate_momentum.pulse.current_tokens) as f32;

        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("privacy-aware gauges");
        let json = serde_json::to_string(&snapshot).expect("serialize privacy-aware gauges");
        let debug = format!("{snapshot:?}");

        assert_eq!(
            snapshot.frame.gauge_fractions,
            [
                vm.progress.fraction,
                exact_daily,
                super::daily_overage_marker_fraction(vm.daily_comparison.fraction_of_yesterday,)
                    as f32,
                exact_pace,
            ]
        );
        assert_eq!(
            snapshot.frame.gauge_levels,
            snapshot
                .frame
                .gauge_fractions
                .map(|value| GaugeLevelSnapshot::from_fraction(f64::from(value)))
        );

        for exact in [vm.progress.fraction, exact_daily, exact_pace] {
            let exact = serde_json::to_string(&exact).unwrap();
            assert!(
                !json.contains(&exact),
                "serialized exact live gauge {exact}: {json}"
            );
            assert!(
                !debug.contains(&exact),
                "debug output leaked exact live gauge {exact}: {debug}"
            );
        }
    }

    #[test]
    fn seed_changes_do_not_change_serialized_pet_placement() {
        let mut first = fixture_with_real_pet_art();
        first.pet_render.seed = "sentinel-seed-alpha".to_string();
        let mut second = first.clone();
        second.pet_render.seed = "sentinel-seed-beta".to_string();
        let layout = CompanionLogicalLayout::round(360.0, 360.0);
        let now = datetime!(2026-07-11 12:00 UTC);

        let first = project_snapshot(&first, now, layout).expect("first projection");
        let second = project_snapshot(&second, now, layout).expect("second projection");

        assert_eq!(
            first.frame.pet_anchor_points,
            second.frame.pet_anchor_points
        );
        assert_eq!(first.frame.pet_depth, second.frame.pet_depth);
        let json = serde_json::to_string(&(first, second)).expect("serialize snapshots");
        assert!(!json.contains("sentinel-seed-alpha"));
        assert!(!json.contains("sentinel-seed-beta"));
    }

    #[test]
    fn topology_order_and_fixed_limits_are_deterministic() {
        let mut vm = WatchViewModel::fixture_with_tank_inhabitants_for_age(
            120,
            time::macros::date!(2026 - 07 - 11),
        );
        let canonical = fixture_with_real_pet_art();
        vm.pet_art = canonical.pet_art;
        vm.pet_spans = canonical.pet_spans;
        vm.habitat.earned_props = crate::game::habitat::HABITAT_PROP_CATALOG
            .iter()
            .map(|spec| crate::tui::view_model::EarnedHabitatPropView {
                id: crate::storage::state::HabitatPropId::new(spec.id),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: spec.kind,
                display_priority: spec.display_priority,
                source: crate::storage::state::HabitatPropSource::LifetimeTokens { threshold: 1.0 },
            })
            .collect();
        let now = datetime!(2026-07-11 12:00 UTC);
        let layout = CompanionLogicalLayout::round(360.0, 360.0);

        let first = project_snapshot(&vm, now, layout).expect("first projection");
        let second = project_snapshot(&vm, now, layout).expect("second projection");

        assert_eq!(first, second);
        assert_eq!(first.topology.visible_props.len(), 10);
        assert_eq!(first.topology.visible_tank_inhabitants.len(), 2);
        assert!(first
            .topology
            .visible_props
            .iter()
            .enumerate()
            .all(|(index, prop)| usize::from(prop.stable_order) == index));
        assert!(first
            .topology
            .visible_tank_inhabitants
            .iter()
            .enumerate()
            .all(|(index, inhabitant)| usize::from(inhabitant.stable_order) == index));
    }

    #[test]
    fn short_pet_art_is_padded_to_the_fixed_lattice() {
        let mut vm = fixture_with_real_pet_art();
        vm.pet_art = vec!["✦".to_string(), "".to_string()];
        vm.pet_spans = vec![StyledSegment {
            line: 0,
            start: 0,
            end: 1,
            role: PaletteRoleName::Particle,
        }];

        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("short declared art is padded");

        assert_eq!(snapshot.content.pet_lines.len(), 10);
        assert_eq!(snapshot.content.pet_lines[0].chars().count(), 13);
        assert!(snapshot.content.pet_lines[0].starts_with('✦'));
        assert!(snapshot.content.pet_lines[2..]
            .iter()
            .all(|line| line == "             "));
    }

    #[test]
    fn oversized_pet_art_fails_closed() {
        let mut too_many_rows = fixture_with_real_pet_art();
        too_many_rows.pet_art = vec!["".to_string(); 11];
        assert_eq!(
            project_snapshot(
                &too_many_rows,
                datetime!(2026-07-11 12:00 UTC),
                CompanionLogicalLayout::round(360.0, 360.0),
            ),
            Err(CompanionSceneProjectionError::PetArtTooTall { row_count: 11 })
        );

        let mut too_many_columns = fixture_with_real_pet_art();
        too_many_columns.pet_art = vec!["✦".repeat(14)];
        assert_eq!(
            project_snapshot(
                &too_many_columns,
                datetime!(2026-07-11 12:00 UTC),
                CompanionLogicalLayout::round(360.0, 360.0),
            ),
            Err(CompanionSceneProjectionError::PetArtTooWide { line_index: 0, char_count: 14 })
        );
    }

    #[test]
    fn invalid_pet_role_spans_fail_closed_against_unpadded_rows() {
        let mut vm = fixture_with_real_pet_art();
        vm.pet_art = vec!["✦".to_string()];
        vm.pet_spans = vec![StyledSegment {
            line: 0,
            start: 0,
            end: 2,
            role: PaletteRoleName::Particle,
        }];

        assert_eq!(
            project_snapshot(
                &vm,
                datetime!(2026-07-11 12:00 UTC),
                CompanionLogicalLayout::round(360.0, 360.0),
            ),
            Err(CompanionSceneProjectionError::InvalidPetRoleSpan {
                span_index: 0,
                line_index: 0,
                start_char: 0,
                end_char: 2,
                source_char_count: 1,
            })
        );
    }

    #[test]
    fn multibyte_pet_role_spans_use_unicode_scalar_indices() {
        let mut vm = fixture_with_real_pet_art();
        vm.pet_art = vec!["✦✦".to_string()];
        vm.pet_spans = vec![StyledSegment {
            line: 0,
            start: 1,
            end: 2,
            role: PaletteRoleName::Eye,
        }];

        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("multibyte scalar span");

        assert_eq!(snapshot.content.pet_roles[0].line_index, 0);
        assert_eq!(snapshot.content.pet_roles[0].start_char, 1);
        assert_eq!(snapshot.content.pet_roles[0].end_char, 2);
    }

    #[test]
    fn frame_motion_matches_the_shared_round_projection_and_monotonic_clock() {
        let mut vm = fixture_with_real_pet_art();
        vm.progress.rate_per_hour = 50_000_000.0;
        vm.breath_offset_y = 1;
        let wall_time = datetime!(2026-07-11 12:00 UTC);
        let elapsed_ms = 250;
        let layout = CompanionLogicalLayout::round(360.0, 360.0);
        let input = CompanionSceneProjectionInput::round(
            CompanionProjectionClock::new(wall_time, elapsed_ms),
            layout,
            44,
            18,
            crate::round::scene::current_round_motion_clearance(18),
        );

        let snapshot = CompanionSceneSnapshot::project_with_input(&vm, input)
            .expect("project shared round motion");
        let shared = crate::round::motion::project_round_companion_motion(
            super::companion_motion_input(
                &vm,
                wall_time,
                &crate::round::motion::companion_roam_motion(),
            ),
            wall_time,
            elapsed_ms,
            input.motion_viewport(),
            &crate::round::motion::companion_roam_motion(),
        );

        assert_eq!(snapshot.frame.elapsed_ms, elapsed_ms);
        assert_eq!(
            snapshot.frame.pet_anchor_points,
            shared.motion_top_left_points
        );
        assert_eq!(snapshot.frame.pet_depth, shared.normalized_depth);
        assert_eq!(snapshot.frame.facing, shared.facing);
        assert_eq!(
            snapshot.frame.breath_offset_y_points,
            f32::from(shared.breath_offset_y_cells) * 20.0
        );
        assert_eq!(
            snapshot.frame.bob_offset_y_points,
            shared.bob_offset_y_cells * 20.0
        );
        assert_ne!(snapshot.frame.bob_offset_y_points, 0.0);
        assert_ne!(snapshot.frame.pet_depth, 0.0);
    }

    #[test]
    fn prop_animation_states_use_authored_cadences_and_explicit_static_state() {
        let mut vm = fixture_with_real_pet_art();
        vm.habitat
            .earned_props
            .push(crate::tui::view_model::EarnedHabitatPropView {
                id: crate::storage::state::HabitatPropId::new(
                    crate::game::habitat::TOKEN_SPARK_500K,
                ),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: crate::game::habitat::HabitatPropKind::Accent,
                display_priority: 30,
                source: crate::storage::state::HabitatPropSource::LifetimeTokens {
                    threshold: 500_000.0,
                },
            });
        vm.habitat
            .earned_props
            .push(crate::tui::view_model::EarnedHabitatPropView {
                id: crate::storage::state::HabitatPropId::new(
                    crate::game::habitat::FIRST_ENSEMBLE_DAY,
                ),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: crate::game::habitat::HabitatPropKind::Trophy,
                display_priority: 65,
                source: crate::storage::state::HabitatPropSource::ActivityMilestone {
                    milestone: "ensemble".to_string(),
                },
            });
        let at = |seconds| {
            project_snapshot(
                &vm,
                time::OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(seconds),
                CompanionLogicalLayout::round(360.0, 360.0),
            )
            .expect("project prop animation state")
        };
        let initial = at(0);
        let sprite_changed = at(4);
        let twinkle_changed = at(2);
        let motion_changed = at(10);

        assert_ne!(
            prop_animation_state(&initial, crate::game::habitat::CODEX_SIGNAL_LAMP).sprite_phase,
            prop_animation_state(&sprite_changed, crate::game::habitat::CODEX_SIGNAL_LAMP)
                .sprite_phase
        );
        assert_ne!(
            prop_animation_state(&initial, crate::game::habitat::TOKEN_SPARK_500K).twinkle_active,
            prop_animation_state(&twinkle_changed, crate::game::habitat::TOKEN_SPARK_500K)
                .twinkle_active
        );
        assert_ne!(
            prop_animation_state(&initial, crate::game::habitat::TOKEN_PEBBLE_25K).motion_phase,
            prop_animation_state(&motion_changed, crate::game::habitat::TOKEN_PEBBLE_25K)
                .motion_phase
        );
        assert!(
            prop_animation_state(&initial, crate::game::habitat::FIRST_ENSEMBLE_DAY).is_static()
        );
        let static_json = serde_json::to_string(prop_animation_state(
            &initial,
            crate::game::habitat::FIRST_ENSEMBLE_DAY,
        ))
        .unwrap();
        assert!(static_json.contains("\"kind\":\"static\""));
        vm.habitat.earned_props.reverse();
        let reordered = project_snapshot(
            &vm,
            time::OffsetDateTime::UNIX_EPOCH,
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("reordered prop projection");
        assert_eq!(
            initial.content.prop_animation_states,
            reordered.content.prop_animation_states
        );
    }

    #[test]
    fn tank_animation_states_are_bounded_identity_stable_and_calm_aware() {
        let mut vm = WatchViewModel::fixture_with_tank_inhabitants_for_age(
            120,
            time::macros::date!(2026 - 07 - 11),
        );
        let canonical = fixture_with_real_pet_art();
        vm.pet_art = canonical.pet_art;
        vm.pet_spans = canonical.pet_spans;
        let now = datetime!(2026-07-11 12:00 UTC);
        let normal = project_snapshot(&vm, now, CompanionLogicalLayout::round(360.0, 360.0))
            .expect("normal tank projection");
        let mut asleep_vm = vm.clone();
        asleep_vm.day_context.asleep = true;
        let asleep = project_snapshot(&asleep_vm, now, CompanionLogicalLayout::round(360.0, 360.0))
            .expect("asleep tank projection");
        vm.life_profile.calm_mode = true;
        let calm = project_snapshot(&vm, now, CompanionLogicalLayout::round(360.0, 360.0))
            .expect("calm tank projection");

        assert_eq!(normal.content.tank_animation_states.len(), 2);
        for state in &normal.content.tank_animation_states {
            assert!(state.sprite_variant <= 1);
            assert_eq!(state.cadence_ms, 4_000);
        }
        assert!(calm
            .content
            .tank_animation_states
            .iter()
            .all(|state| state.calm && state.cadence_ms == 8_000));
        assert!(asleep
            .content
            .tank_animation_states
            .iter()
            .all(|state| state.calm && state.cadence_ms == 8_000));

        vm.habitat.earned_inhabitants.reverse();
        let reordered = project_snapshot(&vm, now, CompanionLogicalLayout::round(360.0, 360.0))
            .expect("reordered tank projection");
        assert_eq!(
            calm.content.tank_animation_states,
            reordered.content.tank_animation_states
        );
    }

    #[test]
    fn tank_state_serializes_visible_route_outcomes_not_seed_tokens() {
        let mut vm = WatchViewModel::fixture_with_tank_inhabitants_for_age(
            120,
            time::macros::date!(2026 - 07 - 11),
        );
        let canonical = fixture_with_real_pet_art();
        vm.pet_art = canonical.pet_art;
        vm.pet_spans = canonical.pet_spans;

        let snapshot = project_snapshot(
            &vm,
            datetime!(2026-07-11 12:00 UTC),
            CompanionLogicalLayout::round(360.0, 360.0),
        )
        .expect("tank route projection");
        let json = serde_json::to_string(&snapshot.content.tank_animation_states)
            .expect("serialize tank outcomes");

        assert!(
            !json.contains("route_phase"),
            "serialized a seed-derived phase: {json}"
        );
        assert!(!json.contains("token"), "serialized a token: {json}");
        assert!(!json.contains("hash"), "serialized a hash: {json}");
        assert!(
            json.contains("origin_col"),
            "missing visible route column: {json}"
        );
        assert!(
            json.contains("origin_row"),
            "missing visible route row: {json}"
        );
        assert!(
            json.contains("sprite_variant"),
            "missing bounded sprite variant: {json}"
        );
        assert!(
            json.contains("color_srgb8"),
            "missing resolved authored tank color: {json}"
        );
        assert!(
            json.contains("\"bold\":true"),
            "missing resolved authored tank weight: {json}"
        );
        assert!(
            json.contains("cells"),
            "missing exact visible cells: {json}"
        );
        assert!(
            json.contains("bounds"),
            "missing exact visible bounds: {json}"
        );
        for state in &snapshot.content.tank_animation_states {
            assert_eq!(state.visible, !state.cells.is_empty());
            assert_eq!(state.bounds.is_some(), state.visible);
        }
    }

    #[test]
    fn shared_tank_route_resolver_fails_closed_and_bounds_catalog_outcomes() {
        let geometry = crate::presentation::tank_life::TankRouteGeometry::round(44, 18, 5);
        for spec in crate::game::habitat::TANK_INHABITANT_CATALOG {
            let outcome = crate::presentation::tank_life::resolve_tank_route(
                crate::presentation::tank_life::TankRouteInput {
                    catalog_id: spec.id,
                    pet_seed: "private-route-seed",
                    local_date: time::macros::date!(2026 - 07 - 11),
                    now: datetime!(2026-07-11 12:00 UTC),
                    calm: false,
                    geometry: &geometry,
                },
            )
            .expect("known catalog route");
            assert!(outcome.sprite_variant <= 1);
            assert!(outcome.origin_col < 44);
            assert!(outcome.origin_row < 18);
        }
        assert!(crate::presentation::tank_life::resolve_tank_route(
            crate::presentation::tank_life::TankRouteInput {
                catalog_id: "private-unknown-inhabitant",
                pet_seed: "private-route-seed",
                local_date: time::macros::date!(2026 - 07 - 11),
                now: datetime!(2026-07-11 12:00 UTC),
                calm: false,
                geometry: &geometry,
            },
        )
        .is_none());
    }

    #[test]
    fn weather_changes_content_without_changing_topology() {
        let clear = fixture_with_real_pet_art();
        let mut sparks = clear.clone();
        sparks.life_profile.work_weather = crate::tui::life::WorkWeather::OutputSparks;
        let now = datetime!(2026-07-11 12:00 UTC);
        let clear = project_snapshot(&clear, now, CompanionLogicalLayout::round(360.0, 360.0))
            .expect("clear projection");
        let sparks = project_snapshot(&sparks, now, CompanionLogicalLayout::round(360.0, 360.0))
            .expect("sparks projection");

        assert_eq!(clear.topology, sparks.topology);
        assert_ne!(clear.content.room_weather, sparks.content.room_weather);
        assert_eq!(
            serde_json::to_string(&clear.topology).unwrap(),
            serde_json::to_string(&sparks.topology).unwrap()
        );
    }

    #[test]
    fn room_weather_and_recent_activity_use_independent_canonical_sources() {
        let now = datetime!(2026-07-11 12:00 UTC);
        let layout = CompanionLogicalLayout::round(360.0, 360.0);
        let weather_cases = [
            (crate::tui::life::WorkWeather::Clear, "clear"),
            (crate::tui::life::WorkWeather::CacheMist, "cache-mist"),
            (crate::tui::life::WorkWeather::OutputSparks, "output-sparks"),
            (
                crate::tui::life::WorkWeather::ReasoningPulse,
                "reasoning-pulse",
            ),
            (crate::tui::life::WorkWeather::Mixed, "mixed"),
        ];

        for (weather, alias) in weather_cases {
            let mut vm = fixture_with_real_pet_art();
            vm.life_profile.work_weather = weather;
            vm.last_feed_pulse_at = Some(now - time::Duration::milliseconds(617));
            let snapshot = project_snapshot(&vm, now, layout).expect("weather projection");

            assert_eq!(snapshot.content.room_weather, alias);
            assert_eq!(
                super::derive_room_profile(&vm, now).room_weather,
                weather.into()
            );
            let profile = super::derive_room_profile(&vm, now);
            let projection_input = CompanionSceneProjectionInput::round(
                CompanionProjectionClock::new(now, 0),
                layout,
                44,
                18,
                crate::round::scene::current_round_motion_clearance(18),
            );
            let roam_motion = crate::round::motion::companion_roam_motion();
            let motion = crate::round::motion::project_round_companion_motion_with_options(
                super::companion_motion_input(&vm, now, &roam_motion),
                now,
                0,
                projection_input.motion_viewport(),
                &roam_motion,
                crate::round::motion::RoundMotionProjectionOptions { depth_override: None },
            );
            let authoritative = crate::tui::room::companion_room_glyphs_for(
                &profile,
                crate::tui::room::CompanionRoomProjectionInput {
                    pet_art: &vm.pet_art,
                    speech_visible: vm.current_speech.is_some(),
                    day_phase: vm.day_context.day_phase,
                    columns: 44,
                    rows: 18,
                    classic_pet_top_left: motion.classic_top_left_cells,
                    pet_frame_extent: [PET_LATTICE_WIDTH, PET_LATTICE_HEIGHT],
                    facing: motion.facing,
                    now,
                },
            )
            .expect("authoritative room projection");
            assert!(!authoritative.is_empty());
            assert_eq!(snapshot.content.room_glyphs.len(), authoritative.len());
            for ((content, frame), expected) in snapshot
                .content
                .room_glyphs
                .iter()
                .zip(&snapshot.frame.room_glyphs)
                .zip(authoritative)
            {
                assert_eq!(content.glyph, expected.glyph);
                assert_eq!(content.color_srgb8, expected.color_rgb);
                assert_eq!(frame.grid_cell, [expected.col, expected.row]);
            }

            assert_eq!(
                snapshot.content.ambient_semantics.len(),
                super::super::scene::MAX_AMBIENT_INSTANCES
            );
            assert_eq!(
                snapshot.frame.ambient_instances.len(),
                super::super::scene::MAX_AMBIENT_INSTANCES
            );
            for (slot, (semantic, frame)) in snapshot
                .content
                .ambient_semantics
                .iter()
                .zip(&snapshot.frame.ambient_instances)
                .enumerate()
            {
                assert_eq!(usize::from(semantic.slot), slot);
                assert_eq!(usize::from(frame.slot), slot);
                assert_eq!(semantic.kind, None);
                assert_eq!(semantic.glyph, None);
                assert!(!frame.visible);
                assert_eq!(frame.position_points, [0.0; 2]);
                assert_eq!(frame.opacity, 0.0);
            }

            let expected_opacity = 1.0 - 617.0 / 2_000.0;
            assert_eq!(
                super::super::canonical_activity_status(&snapshot),
                (true, expected_opacity)
            );

            let mut unrelated_ambient = snapshot.clone();
            unrelated_ambient.content.ambient_semantics.reverse();
            unrelated_ambient.content.ambient_semantics.truncate(1);
            unrelated_ambient.frame.ambient_instances.rotate_left(7);
            unrelated_ambient.frame.ambient_instances.truncate(3);
            assert_eq!(
                super::super::canonical_activity_status(&unrelated_ambient),
                (true, expected_opacity),
                "ambient order and count must not own chrome status"
            );
        }
    }

    #[test]
    fn activity_status_serialization_and_debug_redact_the_exact_fade() {
        let now = datetime!(2026-07-11 12:00 UTC);
        let mut vm = fixture_with_real_pet_art();
        vm.last_feed_pulse_at = Some(now - time::Duration::milliseconds(617));
        let snapshot = project_snapshot(&vm, now, CompanionLogicalLayout::round(360.0, 360.0))
            .expect("recent activity projection");
        let exact_opacity = 1.0 - 617.0 / 2_000.0;
        let exact_opacity = serde_json::to_string(&exact_opacity).unwrap();
        let json = serde_json::to_string(&snapshot).unwrap();
        let debug = format!("{snapshot:?}");

        assert!(json.contains("\"activity_recent\":true"));
        assert!(!json.contains("activity_pulse_age_ms"));
        assert!(!json.contains(&exact_opacity));
        assert!(!debug.contains("activity_pulse_age_ms"));
        assert!(!debug.contains(&exact_opacity));
    }

    #[test]
    fn frame_depth_parity_covers_far_neutral_and_near_fixtures() {
        let vm = fixture_with_real_pet_art();
        let now = datetime!(2026-07-11 12:00 UTC);
        for depth in [-1.0, 0.0, 1.0] {
            let input = CompanionSceneProjectionInput::round(
                CompanionProjectionClock::new(now, 500),
                CompanionLogicalLayout::round(360.0, 360.0),
                44,
                18,
                crate::round::scene::current_round_motion_clearance(18),
            )
            .with_depth_override(depth);
            let snapshot = CompanionSceneSnapshot::project_with_input(&vm, input)
                .expect("depth fixture projection");
            let shared = crate::round::motion::project_round_companion_motion_with_options(
                super::companion_motion_input(
                    &vm,
                    now,
                    &crate::round::motion::companion_roam_motion(),
                ),
                now,
                500,
                input.motion_viewport(),
                &crate::round::motion::companion_roam_motion(),
                crate::round::motion::RoundMotionProjectionOptions { depth_override: Some(depth) },
            );
            assert_eq!(snapshot.frame.pet_depth, depth);
            assert_eq!(snapshot.frame.pet_depth, shared.normalized_depth);
            let expected = crate::round::depth::resolve_smooth_depth(
                depth,
                crate::round::depth::depth_lifecycle_scale(false, false),
            )
            .unwrap();
            assert_eq!(snapshot.frame.pet_depth_cue.scale, expected.scale);
            assert_eq!(
                snapshot.frame.pet_depth_cue.y_offset_points_up,
                -expected.perspective_y * snapshot.topology.glyph_grid.cell_extent_points[1]
            );
            assert_eq!(snapshot.frame.pet_depth_cue.opacity, expected.atmosphere);
            assert_eq!(snapshot.frame.pet_depth_cue.saturation, 1.0);
            assert_eq!(
                snapshot.frame.pet_anchor_points,
                shared.motion_top_left_points
            );
        }
    }

    #[test]
    fn frame_motion_parity_covers_active_and_asleep_calm_lifecycle() {
        let mut active = fixture_with_real_pet_art();
        active.progress.rate_per_hour = 50_000_000.0;
        let mut resting = active.clone();
        resting.day_context.asleep = true;
        resting.life_profile.calm_mode = true;
        let now = datetime!(2026-07-11 12:00 UTC);
        let input = CompanionSceneProjectionInput::round(
            CompanionProjectionClock::new(now, 750),
            CompanionLogicalLayout::round(360.0, 360.0),
            44,
            18,
            crate::round::scene::current_round_motion_clearance(18),
        );
        let active_snapshot = CompanionSceneSnapshot::project_with_input(&active, input)
            .expect("active motion projection");
        let resting_snapshot = CompanionSceneSnapshot::project_with_input(&resting, input)
            .expect("resting motion projection");
        let active_shared = crate::round::motion::project_round_companion_motion(
            super::companion_motion_input(
                &active,
                now,
                &crate::round::motion::companion_roam_motion(),
            ),
            now,
            750,
            input.motion_viewport(),
            &crate::round::motion::companion_roam_motion(),
        );
        let resting_shared = crate::round::motion::project_round_companion_motion(
            super::companion_motion_input(
                &resting,
                now,
                &crate::round::motion::companion_roam_motion(),
            ),
            now,
            750,
            input.motion_viewport(),
            &crate::round::motion::companion_roam_motion(),
        );

        assert_eq!(
            active_snapshot.frame.pet_anchor_points,
            active_shared.motion_top_left_points
        );
        assert_eq!(
            resting_snapshot.frame.pet_anchor_points,
            resting_shared.motion_top_left_points
        );
        assert_eq!(
            resting_snapshot.frame.bob_offset_y_points,
            active_snapshot.frame.bob_offset_y_points
        );
        assert!(resting_snapshot.frame.asleep);
        assert!(resting_snapshot
            .content
            .tank_animation_states
            .iter()
            .all(|state| state.calm));
        assert!(resting_shared.normalized_depth.abs() < active_shared.normalized_depth.abs());
    }
}
