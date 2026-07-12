use super::{
    AuthoredDepthSnapshot, CompanionSceneProjectionError, CompanionSceneProjectionInput,
    CompanionSceneSnapshot, ContentSnapshot, FrameSnapshot, GaugeLevelSnapshot, PaletteSnapshot,
    PetLatticeSnapshot, PetRoleSpanSnapshot, PetTopologySnapshot, PropAnimationKindSnapshot,
    PropAnimationSnapshot, PropTopologySnapshot, PropZoneSnapshot, RoomTopologySnapshot,
    TankAnimationSnapshot, TankLayerSnapshot, TankRouteSnapshot, TankSideSnapshot,
    TankTopologySnapshot, TopologySnapshot, COMPANION_RENDERER_SCHEMA_VERSION,
    COMPANION_SCENE_SCHEMA_VERSION, MAX_VISIBLE_PROPS, MAX_VISIBLE_TANK_INHABITANTS,
    PET_LATTICE_HEIGHT, PET_LATTICE_SLOTS, PET_LATTICE_WIDTH,
};
use crate::game::habitat::{HabitatPetLayer, HabitatPropZone, TankLifeRouteFamily};
use crate::pet::palette::{body_glow, ResolvedPalette, Rgb};
use crate::pet::render::{PaletteRoleName, StyledSegment};
use crate::presentation::privacy::{PresentationSurface, PrivacyProjection};
use crate::round::hud::{
    companion_pace_fraction, daily_fraction_for_gauge, daily_overage_marker_fraction,
};
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
        let prop_animation_states = project_prop_animation_states(&visible_props, now);
        let tank_animation_states =
            project_tank_animation_states(vm, &visible_tank_inhabitants, input);
        let hud = crate::round::hud::review_capture_hud_text();
        let asleep = vm.day_context.asleep;
        let dimmed = asleep || vm.life_profile.calm_mode;

        Ok(Self {
            schema_version: COMPANION_SCENE_SCHEMA_VERSION,
            privacy: PrivacyProjection::for_surface(PresentationSurface::RoundCompanion),
            topology: TopologySnapshot {
                layout,
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
                pet_lines,
                pet_roles,
                palette: PaletteSnapshot::from(vm.pet_palette),
                prop_animation_states,
                tank_animation_states,
                activity_pulse_age_ms: activity_pulse.age_ms(),
            },
            frame: FrameSnapshot {
                elapsed_ms,
                pet_anchor_points: motion.motion_top_left_points,
                pet_depth: motion.normalized_depth,
                facing: motion.facing,
                breath_offset_y_cells: motion.breath_offset_y_cells,
                bob_offset_y_cells: motion.bob_offset_y_cells,
                asleep,
                helper_trouble: helper_health == SemanticHelperHealth::Trouble,
                gauges: [
                    gauge_level(if vm.progress.is_max_stage {
                        1.0
                    } else {
                        f64::from(vm.progress.fraction)
                    }),
                    gauge_level(daily_fraction_for_gauge(
                        vm.daily_comparison.fraction_of_yesterday,
                    )),
                    gauge_level(daily_overage_marker_fraction(
                        vm.daily_comparison.fraction_of_yesterday,
                    )),
                    gauge_level(companion_pace_fraction(
                        vm.rate_momentum.pulse.current_tokens,
                    )),
                ],
                dim_amount: if dimmed { 0.35 } else { 0.0 },
                hud_lines: [hud.today_total, hud.daily_percent, hud.pace],
            },
        })
    }
}

fn gauge_level(value: f64) -> GaugeLevelSnapshot {
    let value = if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if value == 0.0 {
        GaugeLevelSnapshot::Empty
    } else if value < 0.25 {
        GaugeLevelSnapshot::Low
    } else if value < 0.5 {
        GaugeLevelSnapshot::Medium
    } else if value < 1.0 {
        GaugeLevelSnapshot::High
    } else {
        GaugeLevelSnapshot::Full
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
    visible_props: &[PropTopologySnapshot],
    now: OffsetDateTime,
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
            }
        })
        .collect()
}

fn project_tank_animation_states(
    vm: &WatchViewModel,
    visible_tank_inhabitants: &[TankTopologySnapshot],
    input: CompanionSceneProjectionInput,
) -> Vec<TankAnimationSnapshot> {
    let calm = vm.life_profile.calm_mode || vm.day_context.asleep;
    let geometry = crate::presentation::tank_life::TankRouteGeometry::round(
        input.grid_columns,
        input.grid_rows,
        input.motion_clearance.bottom_reserved_rows,
    );
    visible_tank_inhabitants
        .iter()
        .filter_map(|inhabitant| {
            let outcome = crate::presentation::tank_life::resolve_tank_route(
                crate::presentation::tank_life::TankRouteInput {
                    catalog_id: inhabitant.catalog_id,
                    pet_seed: &vm.pet_render.seed,
                    local_date: vm.habitat.tank_life_local_date,
                    now: input.clock.wall_time,
                    calm,
                    geometry: &geometry,
                },
            )?;
            Some(TankAnimationSnapshot {
                catalog_id: inhabitant.catalog_id,
                stable_order: inhabitant.stable_order,
                route: outcome.route.into(),
                visible: outcome.visible,
                origin_col: outcome.origin_col,
                origin_row: outcome.origin_row,
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
                cadence_ms: outcome.cadence_ms,
                calm: outcome.calm,
            })
        })
        .collect()
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
