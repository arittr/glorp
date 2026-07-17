use crate::game::habitat::{
    HabitatPetLayer, HabitatPropKind, HabitatPropShadowProfile, CODEX_SIGNAL_LAMP,
    FIRST_ENSEMBLE_DAY, HEAVY_SESSION_PLANTER, RETURN_SPROUT, TOKEN_AURORA_500M, TOKEN_BONSAI_100M,
    TOKEN_CONSTELLATION_250M, TOKEN_FRIENDLY_CLOUD_750K, TOKEN_GEODE_50M, TOKEN_HANGING_VINE_25M,
    TOKEN_LANTERN_10M, TOKEN_MOON_1B, TOKEN_MOSS_TUFT_250K, TOKEN_ORBIT_5M, TOKEN_PEBBLE_25K,
    TOKEN_REEDS_5M, TOKEN_SHARD_1M, TOKEN_SHELL_100K, TOKEN_SPARK_500K, TOKEN_TREASURE_CHEST_2M,
    WILT_RECOVERY_SPROUT,
};
use crate::pet::generation::Species;
use crate::presentation::target::SurfaceTargetId;
use crate::tui::component::habitat_props::{HabitatPropCell, HabitatPropPlacement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PresentationPropVisualState {
    pub(crate) species: Species,
    pub(crate) sprite_phase: Option<u8>,
    pub(crate) twinkle_active: Option<bool>,
    pub(crate) chest_lid_open: Option<bool>,
    pub(crate) bloom_active: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PresentationPropSpriteCell {
    pub(crate) dx: i8,
    pub(crate) dy: i8,
    pub(crate) glyph: char,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PresentationPropFootprint {
    pub(crate) min_dx: i8,
    pub(crate) max_dx: i8,
    pub(crate) min_dy: i8,
    pub(crate) max_dy: i8,
}

// These shared interfaces are consumed incrementally by prop-shadow Tasks 2 and 4.
#[allow(dead_code, reason = "consumed by follow-up prop-shadow tasks")]
pub(crate) const PROP_CAST_SHADOW_DIRECTION_Y_UP: [f32; 2] = [0.196_116_13, -0.980_580_7];

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code, reason = "consumed by follow-up prop-shadow tasks")]
pub(crate) struct PropShadowResolveInput {
    pub(crate) profile: HabitatPropShadowProfile,
    pub(crate) visible: bool,
    pub(crate) grounded: bool,
    pub(crate) opacity: f32,
    pub(crate) footprint_points: [f32; 2],
    pub(crate) cell_extent_points: [f32; 2],
    pub(crate) contact_strength: f32,
    pub(crate) origin_y_up_points: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedPropCastShadow {
    pub(crate) vector_y_up_points: [f32; 2],
    pub(crate) softness_points: f32,
    pub(crate) strength: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedPropShadow {
    pub(crate) origin_y_up_points: [f32; 2],
    pub(crate) footprint_points: [f32; 2],
    pub(crate) cell_extent_points: [f32; 2],
    pub(crate) opacity: f32,
    pub(crate) contact_strength: f32,
    pub(crate) cast: Option<ResolvedPropCastShadow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code, reason = "consumed by follow-up prop-shadow tasks")]
pub(crate) enum PropShadowResolveError {
    NonFinite,
    NegativeGeometry,
    InvalidAuthoredProfile,
}

#[allow(dead_code, reason = "consumed by follow-up prop-shadow tasks")]
pub(crate) fn resolve_prop_shadow(
    input: PropShadowResolveInput,
) -> Result<ResolvedPropShadow, PropShadowResolveError> {
    let runtime_values = [
        input.opacity,
        input.footprint_points[0],
        input.footprint_points[1],
        input.cell_extent_points[0],
        input.cell_extent_points[1],
        input.contact_strength,
        input.origin_y_up_points[0],
        input.origin_y_up_points[1],
    ];
    if runtime_values.into_iter().any(|value| !value.is_finite()) {
        return Err(PropShadowResolveError::NonFinite);
    }
    if !(0.0..=1.0).contains(&input.opacity)
        || input.footprint_points.into_iter().any(|value| value < 0.0)
        || input
            .cell_extent_points
            .into_iter()
            .any(|value| value <= 0.0)
        || !(0.0..=1.0).contains(&input.contact_strength)
    {
        return Err(PropShadowResolveError::NegativeGeometry);
    }

    let elevated = match input.profile {
        HabitatPropShadowProfile::None | HabitatPropShadowProfile::ContactOnly => None,
        HabitatPropShadowProfile::Elevated { visual_height_cells, softness_cells } => {
            if !visual_height_cells.is_finite() || !softness_cells.is_finite() {
                return Err(PropShadowResolveError::NonFinite);
            }
            if visual_height_cells <= 0.0 || softness_cells <= 0.0 {
                return Err(PropShadowResolveError::InvalidAuthoredProfile);
            }
            Some((visual_height_cells, softness_cells))
        }
    };

    let emits_ink = input.visible && input.grounded && input.opacity > 0.0;
    let contact_strength = if !emits_ink {
        0.0
    } else if elevated.is_some() {
        input.contact_strength.max(0.22)
    } else {
        input.contact_strength
    };
    let cast = elevated.and_then(|(visual_height_cells, softness_cells)| {
        let large_enough = input.footprint_points[0] >= input.cell_extent_points[0]
            && input.footprint_points[1] >= input.cell_extent_points[1] * 2.0;
        if !emits_ink || !large_enough {
            return None;
        }

        let length_cells = (visual_height_cells * 0.55).min(4.0);
        let length_points = length_cells * input.cell_extent_points[1];
        let softness_points = (softness_cells * input.cell_extent_points[1] + length_points * 0.12)
            .clamp(
                input.cell_extent_points[1] * 0.20,
                input.cell_extent_points[1] * 1.25,
            );
        let strength = contact_strength * 0.65 * (1.0 - 0.08 * length_cells / 4.0);
        Some(ResolvedPropCastShadow {
            vector_y_up_points: [
                PROP_CAST_SHADOW_DIRECTION_Y_UP[0] * length_points,
                PROP_CAST_SHADOW_DIRECTION_Y_UP[1] * length_points,
            ],
            softness_points,
            strength,
        })
    });

    Ok(ResolvedPropShadow {
        origin_y_up_points: input.origin_y_up_points,
        footprint_points: input.footprint_points,
        cell_extent_points: input.cell_extent_points,
        opacity: input.opacity,
        contact_strength,
        cast,
    })
}

#[allow(dead_code, reason = "consumed by follow-up prop-shadow tasks")]
fn prop_shadow_coverage_at(point_y_up: [f32; 2], shadow: ResolvedPropShadow) -> f32 {
    if point_y_up.into_iter().any(|value| !value.is_finite()) {
        return 0.0;
    }
    let footprint = shadow.footprint_points;
    let cell_extent = shadow.cell_extent_points;
    let contact_radii = [
        (footprint[0] * 0.375).max(cell_extent[0]),
        cell_extent[1] * 0.15,
    ];
    let contact_center = [
        shadow.origin_y_up_points[0] + footprint[0] * 0.5,
        shadow.origin_y_up_points[1] - (footprint[1] - cell_extent[1]).max(0.0) + contact_radii[1],
    ];
    let contact_distance = ((point_y_up[0] - contact_center[0]) / contact_radii[0])
        .hypot((point_y_up[1] - contact_center[1]) / contact_radii[1]);
    let contact_coverage = if contact_distance <= 1.0 {
        shadow.contact_strength * shadow.opacity
    } else {
        0.0
    };

    let cast_coverage = shadow.cast.map_or(0.0, |cast| {
        let segment_length_squared = cast.vector_y_up_points[0].mul_add(
            cast.vector_y_up_points[0],
            cast.vector_y_up_points[1].powi(2),
        );
        if segment_length_squared <= 0.0 {
            return 0.0;
        }
        let relative = [
            point_y_up[0] - contact_center[0],
            point_y_up[1] - contact_center[1],
        ];
        let along = ((relative[0] * cast.vector_y_up_points[0]
            + relative[1] * cast.vector_y_up_points[1])
            / segment_length_squared)
            .clamp(0.0, 1.0);
        let distance = (relative[0] - cast.vector_y_up_points[0] * along)
            .hypot(relative[1] - cast.vector_y_up_points[1] * along);
        let core_radius = (footprint[0] * 0.25).max(cell_extent[0] * 0.5);
        let falloff = 1.0 - smoothstep(core_radius, core_radius + cast.softness_points, distance);
        falloff * cast.strength * shadow.opacity
    });

    contact_coverage.max(cast_coverage).clamp(0.0, 1.0)
}

#[allow(dead_code, reason = "consumed by follow-up prop-shadow tasks")]
fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[allow(dead_code, reason = "consumed by follow-up prop-shadow tasks")]
pub(crate) fn prop_shadow_union_coverage(
    point_y_up: [f32; 2],
    shadows: &[ResolvedPropShadow],
) -> f32 {
    shadows.iter().copied().fold(0.0, |coverage, shadow| {
        coverage.max(prop_shadow_coverage_at(point_y_up, shadow))
    })
}

pub(crate) fn presentation_unknown_prop_sprite(
    kind: HabitatPropKind,
) -> Vec<PresentationPropSpriteCell> {
    match kind {
        HabitatPropKind::Trophy => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '◈' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '▝' },
        ],
        HabitatPropKind::Accent => {
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '·' }]
        }
    }
}

pub(crate) fn presentation_prop_sprite(
    catalog_id: &str,
    state: PresentationPropVisualState,
) -> Option<Vec<PresentationPropSpriteCell>> {
    let first_sprite_phase = state.sprite_phase == Some(0);
    let bloomed = state.bloom_active == Some(true);
    let twinkle = state.twinkle_active == Some(true);
    let sprite = match catalog_id {
        TOKEN_PEBBLE_25K => vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '▲' }],
        TOKEN_SHELL_100K => vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '◌' }],
        TOKEN_SPARK_500K => vec![PresentationPropSpriteCell {
            dx: 0,
            dy: 0,
            glyph: if twinkle { '✦' } else { '·' },
        }],
        TOKEN_SHARD_1M if matches!(state.species, Species::Glitch) => {
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '#' }]
        }
        TOKEN_SHARD_1M => vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '◆' }],
        TOKEN_ORBIT_5M if matches!(state.species, Species::Glitch) => {
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: ']' }]
        }
        TOKEN_ORBIT_5M => vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '°' }],
        TOKEN_LANTERN_10M if matches!(state.species, Species::Glitch) && twinkle => {
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '_' }]
        }
        TOKEN_LANTERN_10M if matches!(state.species, Species::Glitch) => {
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: ':' }]
        }
        TOKEN_LANTERN_10M if matches!(state.species, Species::Crystal) && twinkle => {
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '✦' }]
        }
        TOKEN_LANTERN_10M if twinkle => {
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '☼' }]
        }
        TOKEN_LANTERN_10M => vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '○' }],
        // Bloomed plants flower once matured in the tank. The blossoms (*)
        // twinkle between phases while paint adapters retain blossom coloring.
        TOKEN_MOSS_TUFT_250K if bloomed && first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '▂' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '▃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '▂' },
        ],
        TOKEN_MOSS_TUFT_250K if bloomed => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '▃' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '▂' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '▃' },
        ],
        TOKEN_HANGING_VINE_25M if bloomed && first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '╽' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╱' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '*' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '╲' },
        ],
        TOKEN_HANGING_VINE_25M if bloomed => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '╽' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '*' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '╱' },
        ],
        HEAVY_SESSION_PLANTER if bloomed && first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╱' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '◌' },
        ],
        HEAVY_SESSION_PLANTER if bloomed => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╱' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '◌' },
        ],
        TOKEN_MOSS_TUFT_250K if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '▂' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '▃' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '▂' },
        ],
        TOKEN_MOSS_TUFT_250K => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '▃' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '▂' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '▃' },
        ],
        TOKEN_FRIENDLY_CLOUD_750K if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '☁' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '◦' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '◡' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '◦' },
        ],
        TOKEN_FRIENDLY_CLOUD_750K => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '☁' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '˙' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '◡' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '˙' },
        ],
        // Treasure chest: lid open (gap in the lid + a ✦ glint) during the
        // bubble cycle's open window, closed (solid lid + ◆) otherwise.
        TOKEN_TREASURE_CHEST_2M if state.chest_lid_open == Some(true) => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '✦' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '╱' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '▣' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '◆' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '▣' },
        ],
        TOKEN_TREASURE_CHEST_2M => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╭' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '─' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '╮' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '▣' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '◆' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '▣' },
        ],
        TOKEN_HANGING_VINE_25M if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '╽' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╱' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '╲' },
        ],
        TOKEN_HANGING_VINE_25M => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '╽' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '╱' },
        ],
        CODEX_SIGNAL_LAMP if matches!(state.species, Species::Glitch) && first_sprite_phase => {
            vec![
                PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╷' },
                PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '#' },
                PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '_' },
            ]
        }
        CODEX_SIGNAL_LAMP if matches!(state.species, Species::Glitch) => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '_' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: ':' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╵' },
        ],
        CODEX_SIGNAL_LAMP if matches!(state.species, Species::Crystal) && first_sprite_phase => {
            vec![
                PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╷' },
                PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '◆' },
                PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╵' },
            ]
        }
        CODEX_SIGNAL_LAMP if matches!(state.species, Species::Crystal) => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╷' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '◇' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╵' },
        ],
        CODEX_SIGNAL_LAMP if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╷' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '◉' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╵' },
        ],
        CODEX_SIGNAL_LAMP => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╷' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '○' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '╵' },
        ],
        HEAVY_SESSION_PLANTER if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: 'ѱ' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╱' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '◌' },
        ],
        HEAVY_SESSION_PLANTER => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: 'ѱ' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╱' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '◌' },
        ],
        WILT_RECOVERY_SPROUT if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '╿' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╱' },
        ],
        WILT_RECOVERY_SPROUT => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '╿' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╱' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╲' },
        ],
        // Amethyst geode: a 3×3 facet cluster in a rock cradle that shimmers.
        TOKEN_GEODE_50M if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '◆' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '◇' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '◆' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '◇' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '◈' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '◇' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '◣' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '▼' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '◢' },
        ],
        TOKEN_GEODE_50M => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '◇' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '◆' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '◇' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '◆' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '✦' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '◆' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '◣' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '▼' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '◢' },
        ],
        TOKEN_BONSAI_100M if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '▓' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╱' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '▂' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '▃' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '▂' },
        ],
        TOKEN_BONSAI_100M => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '▓' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '▓' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╲' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╱' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '▂' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '▃' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '▂' },
        ],
        TOKEN_CONSTELLATION_250M if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '✦' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '✦' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '·' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '*' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '·' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '✦' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '·' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '✦' },
        ],
        TOKEN_CONSTELLATION_250M => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '✦' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '✦' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '*' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '✦' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '·' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '✦' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '·' },
        ],
        TOKEN_AURORA_500M if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '✦' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 4, dy: 0, glyph: '✦' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╿' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╿' },
            PresentationPropSpriteCell { dx: 4, dy: 1, glyph: '╿' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '┊' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '┊' },
            PresentationPropSpriteCell { dx: 4, dy: 2, glyph: '┊' },
        ],
        TOKEN_AURORA_500M => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '✦' },
            PresentationPropSpriteCell { dx: 4, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '╿' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '╿' },
            PresentationPropSpriteCell { dx: 4, dy: 1, glyph: '╿' },
            PresentationPropSpriteCell { dx: 0, dy: 2, glyph: '┊' },
            PresentationPropSpriteCell { dx: 2, dy: 2, glyph: '┊' },
            PresentationPropSpriteCell { dx: 4, dy: 2, glyph: '┊' },
        ],
        TOKEN_MOON_1B if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '·' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '◑' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '·' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '·' },
            PresentationPropSpriteCell { dx: 3, dy: 1, glyph: '✦' },
        ],
        TOKEN_MOON_1B => vec![
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '·' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '·' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '◑' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '·' },
            PresentationPropSpriteCell { dx: 1, dy: 2, glyph: '·' },
            PresentationPropSpriteCell { dx: 3, dy: 1, glyph: '·' },
        ],
        TOKEN_REEDS_5M if bloomed && first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '│' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '╷' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '│' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '│' },
        ],
        TOKEN_REEDS_5M if bloomed => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╷' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '│' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '*' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '│' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '│' },
        ],
        TOKEN_REEDS_5M if first_sprite_phase => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╵' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '│' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '╷' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '│' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '│' },
        ],
        TOKEN_REEDS_5M => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '╷' },
            PresentationPropSpriteCell { dx: 1, dy: 0, glyph: '│' },
            PresentationPropSpriteCell { dx: 2, dy: 0, glyph: '╵' },
            PresentationPropSpriteCell { dx: 0, dy: 1, glyph: '│' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '┃' },
            PresentationPropSpriteCell { dx: 2, dy: 1, glyph: '│' },
        ],
        FIRST_ENSEMBLE_DAY | RETURN_SPROUT => vec![
            PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '◈' },
            PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '▝' },
        ],
        _ => return None,
    };
    Some(sprite)
}

fn presentation_prop_visual_states(catalog_id: &str) -> Option<Vec<PresentationPropVisualState>> {
    crate::game::habitat::catalog_prop_by_str(catalog_id)?;
    let supports_sprite_phase = crate::game::habitat::habitat_prop_animation_state(
        catalog_id,
        time::OffsetDateTime::UNIX_EPOCH,
    )
    .sprite_phase
    .is_some();
    let phases = if supports_sprite_phase {
        &[Some(0), Some(1)][..]
    } else {
        &[None][..]
    };
    let mut states = Vec::with_capacity(Species::all().len() * phases.len() * 8);
    for species in Species::all() {
        for &sprite_phase in phases {
            for twinkle_active in [false, true] {
                for chest_lid_open in [false, true] {
                    for bloom_active in [false, true] {
                        states.push(PresentationPropVisualState {
                            species,
                            sprite_phase,
                            twinkle_active: Some(twinkle_active),
                            chest_lid_open: Some(chest_lid_open),
                            bloom_active: Some(bloom_active),
                        });
                    }
                }
            }
        }
    }
    Some(states)
}

pub(crate) fn presentation_prop_max_footprint(
    catalog_id: &str,
) -> Option<PresentationPropFootprint> {
    let mut footprint = None::<PresentationPropFootprint>;
    for state in presentation_prop_visual_states(catalog_id)? {
        for cell in presentation_prop_sprite(catalog_id, state)? {
            footprint = Some(match footprint {
                Some(current) => PresentationPropFootprint {
                    min_dx: current.min_dx.min(cell.dx),
                    max_dx: current.max_dx.max(cell.dx),
                    min_dy: current.min_dy.min(cell.dy),
                    max_dy: current.max_dy.max(cell.dy),
                },
                None => PresentationPropFootprint {
                    min_dx: cell.dx,
                    max_dx: cell.dx,
                    min_dy: cell.dy,
                    max_dy: cell.dy,
                },
            });
        }
    }
    footprint
}

pub(crate) fn presentation_prop_occupied_offsets(catalog_id: &str) -> Option<Vec<[i8; 2]>> {
    let mut offsets = Vec::new();
    for state in presentation_prop_visual_states(catalog_id)? {
        for cell in presentation_prop_sprite(catalog_id, state)? {
            let offset = [cell.dx, cell.dy];
            if !offsets.contains(&offset) {
                offsets.push(offset);
            }
        }
    }
    offsets.sort_unstable();
    Some(offsets)
}

pub(crate) fn presentation_prop_footprint(
    catalog_id: &str,
    state: PresentationPropVisualState,
) -> Option<PresentationPropFootprint> {
    let sprite = presentation_prop_sprite(catalog_id, state)?;
    let mut cells = sprite.into_iter();
    let first = cells.next()?;
    Some(cells.fold(
        PresentationPropFootprint {
            min_dx: first.dx,
            max_dx: first.dx,
            min_dy: first.dy,
            max_dy: first.dy,
        },
        |footprint, cell| PresentationPropFootprint {
            min_dx: footprint.min_dx.min(cell.dx),
            max_dx: footprint.max_dx.max(cell.dx),
            min_dy: footprint.min_dy.min(cell.dy),
            max_dy: footprint.max_dy.max(cell.dy),
        },
    ))
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresentationPropPlacement {
    pub prop_id: String,
    pub layer: PresentationPropLayer,
    pub bounds: PresentationRect,
    pub cells: Vec<PresentationPropCell>,
    pub effect_target: Option<SurfaceTargetId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationPropLayer {
    Background,
    Behind,
    Foreground,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationPropCell {
    pub x: u16,
    pub y: u16,
    pub glyph: char,
}

impl PresentationPropPlacement {
    pub fn from_habitat_placement(placement: &HabitatPropPlacement) -> Self {
        Self {
            prop_id: placement.prop_id.as_str().to_string(),
            layer: presentation_layer(placement.pet_layer),
            bounds: PresentationRect {
                x: placement.bounds.x,
                y: placement.bounds.y,
                width: placement.bounds.width,
                height: placement.bounds.height,
            },
            cells: placement.cells.iter().map(presentation_cell).collect(),
            effect_target: placement.target_id.map(|target| {
                let raw = target.as_str();
                let neutral = raw.strip_prefix("watch.").unwrap_or(raw);
                SurfaceTargetId::new(neutral.to_string())
            }),
        }
    }
}

fn presentation_layer(layer: HabitatPetLayer) -> PresentationPropLayer {
    match layer {
        HabitatPetLayer::Background => PresentationPropLayer::Background,
        HabitatPetLayer::Behind => PresentationPropLayer::Behind,
        HabitatPetLayer::Foreground => PresentationPropLayer::Foreground,
    }
}

fn presentation_cell(cell: &HabitatPropCell) -> PresentationPropCell {
    PresentationPropCell {
        x: cell.col,
        y: cell.row,
        glyph: cell.glyph,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::habitat::*;
    use crate::pet::generation::Species;

    fn elevated_shadow_input() -> PropShadowResolveInput {
        PropShadowResolveInput {
            profile: HabitatPropShadowProfile::Elevated {
                visual_height_cells: 3.0,
                softness_cells: 0.65,
            },
            visible: true,
            grounded: true,
            opacity: 1.0,
            footprint_points: [18.0, 36.0],
            cell_extent_points: [18.0, 18.0],
            contact_strength: 0.24,
            origin_y_up_points: [100.0, 80.0],
        }
    }

    #[test]
    fn elevated_prop_resolves_fixed_direction_length_softness_and_strength() {
        let resolved = resolve_prop_shadow(elevated_shadow_input()).unwrap();

        assert_eq!(resolved.contact_strength, 0.24);
        assert_eq!(resolved.cell_extent_points, [18.0, 18.0]);
        let cast = resolved.cast.unwrap();
        let length = cast.vector_y_up_points[0].hypot(cast.vector_y_up_points[1]);
        assert!((length - 29.7).abs() < 0.001);
        assert!(
            (cast.vector_y_up_points[0] - PROP_CAST_SHADOW_DIRECTION_Y_UP[0] * 29.7).abs() < 0.001
        );
        assert!(
            (cast.vector_y_up_points[1] - PROP_CAST_SHADOW_DIRECTION_Y_UP[1] * 29.7).abs() < 0.001
        );
        assert!((cast.softness_points - 15.264).abs() < 0.001);
        assert!((cast.strength - 0.150_852).abs() < 0.001);
    }

    #[test]
    fn elevated_cast_eligibility_and_size_suppression_follow_runtime_contract() {
        for input in [
            PropShadowResolveInput {
                visible: false,
                ..elevated_shadow_input()
            },
            PropShadowResolveInput {
                grounded: false,
                ..elevated_shadow_input()
            },
            PropShadowResolveInput { opacity: 0.0, ..elevated_shadow_input() },
        ] {
            let resolved = resolve_prop_shadow(input).unwrap();
            assert_eq!(resolved.contact_strength, 0.0);
            assert!(resolved.cast.is_none());
        }

        for input in [
            PropShadowResolveInput {
                footprint_points: [17.9, 36.0],
                contact_strength: 0.10,
                ..elevated_shadow_input()
            },
            PropShadowResolveInput {
                footprint_points: [18.0, 35.9],
                contact_strength: 0.10,
                ..elevated_shadow_input()
            },
        ] {
            let resolved = resolve_prop_shadow(input).unwrap();
            assert_eq!(resolved.contact_strength, 0.22);
            assert!(resolved.cast.is_none());
        }
    }

    #[test]
    fn none_and_contact_only_preserve_visible_grounded_contact_without_casting() {
        for profile in [
            HabitatPropShadowProfile::None,
            HabitatPropShadowProfile::ContactOnly,
        ] {
            let resolved = resolve_prop_shadow(PropShadowResolveInput {
                profile,
                contact_strength: 0.17,
                ..elevated_shadow_input()
            })
            .unwrap();
            assert_eq!(resolved.contact_strength, 0.17);
            assert!(resolved.cast.is_none());

            let hidden = resolve_prop_shadow(PropShadowResolveInput {
                profile,
                visible: false,
                contact_strength: 0.17,
                ..elevated_shadow_input()
            })
            .unwrap();
            assert_eq!(hidden.contact_strength, 0.0);
            assert!(hidden.cast.is_none());
        }
    }

    #[test]
    fn prop_shadow_resolver_rejects_invalid_runtime_and_authored_floats() {
        assert_eq!(
            resolve_prop_shadow(PropShadowResolveInput {
                opacity: f32::NAN,
                ..elevated_shadow_input()
            }),
            Err(PropShadowResolveError::NonFinite)
        );
        assert_eq!(
            resolve_prop_shadow(PropShadowResolveInput {
                footprint_points: [-1.0, 36.0],
                ..elevated_shadow_input()
            }),
            Err(PropShadowResolveError::NegativeGeometry)
        );
        assert_eq!(
            resolve_prop_shadow(PropShadowResolveInput {
                profile: HabitatPropShadowProfile::Elevated {
                    visual_height_cells: 0.0,
                    softness_cells: 0.65,
                },
                ..elevated_shadow_input()
            }),
            Err(PropShadowResolveError::InvalidAuthoredProfile)
        );
    }

    #[test]
    fn shadow_union_uses_maximum_coverage() {
        let shadow = resolve_prop_shadow(elevated_shadow_input()).unwrap();
        let sample = [112.0, 70.0];
        let single = prop_shadow_union_coverage(sample, &[shadow]);
        let duplicate = prop_shadow_union_coverage(sample, &[shadow, shadow]);
        assert!(single > 0.0);
        assert_eq!(single, duplicate);
    }

    #[test]
    fn unknown_prop_fallback_art_is_owned_by_presentation() {
        assert_eq!(
            presentation_unknown_prop_sprite(HabitatPropKind::Trophy),
            vec![
                PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '◈' },
                PresentationPropSpriteCell { dx: 1, dy: 1, glyph: '▝' },
            ]
        );
        assert_eq!(
            presentation_unknown_prop_sprite(HabitatPropKind::Accent),
            vec![PresentationPropSpriteCell { dx: 0, dy: 0, glyph: '·' }]
        );
    }

    #[test]
    fn canonical_prop_states_fit_their_frozen_footprints() {
        let frozen_footprint = |catalog_id| match catalog_id {
            TOKEN_PEBBLE_25K | TOKEN_SHELL_100K | TOKEN_SPARK_500K | TOKEN_SHARD_1M
            | TOKEN_ORBIT_5M | TOKEN_LANTERN_10M => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 0,
                min_dy: 0,
                max_dy: 0,
            },
            TOKEN_MOSS_TUFT_250K
            | TOKEN_FRIENDLY_CLOUD_750K
            | TOKEN_TREASURE_CHEST_2M
            | TOKEN_REEDS_5M => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 2,
                min_dy: 0,
                max_dy: 1,
            },
            TOKEN_HANGING_VINE_25M
            | TOKEN_GEODE_50M
            | TOKEN_BONSAI_100M
            | TOKEN_CONSTELLATION_250M
            | HEAVY_SESSION_PLANTER => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 2,
                min_dy: 0,
                max_dy: 2,
            },
            TOKEN_AURORA_500M => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 4,
                min_dy: 0,
                max_dy: 2,
            },
            TOKEN_MOON_1B => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 3,
                min_dy: 0,
                max_dy: 2,
            },
            CODEX_SIGNAL_LAMP => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 0,
                min_dy: 0,
                max_dy: 2,
            },
            WILT_RECOVERY_SPROUT => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 2,
                min_dy: 0,
                max_dy: 1,
            },
            FIRST_ENSEMBLE_DAY | RETURN_SPROUT => PresentationPropFootprint {
                min_dx: 0,
                max_dx: 1,
                min_dy: 0,
                max_dy: 1,
            },
            other => panic!("missing frozen footprint for {other}"),
        };

        for spec in HABITAT_PROP_CATALOG {
            let footprint = presentation_prop_max_footprint(spec.id)
                .unwrap_or_else(|| panic!("{} must have a canonical footprint", spec.id));
            assert_eq!(
                footprint,
                frozen_footprint(spec.id),
                "{} footprint",
                spec.id
            );

            let states = presentation_prop_visual_states(spec.id)
                .unwrap_or_else(|| panic!("{} must enumerate visual states", spec.id));
            assert!(
                !states.is_empty(),
                "{} must enumerate visual states",
                spec.id
            );
            for species in Species::all() {
                assert!(states.iter().any(|state| state.species == species));
            }
            for state in states {
                let active_footprint = presentation_prop_footprint(spec.id, state)
                    .unwrap_or_else(|| panic!("{} must have an active footprint", spec.id));
                assert_eq!(active_footprint.min_dx, 0, "{} state {state:?}", spec.id);
                assert_eq!(active_footprint.min_dy, 0, "{} state {state:?}", spec.id);
                let sprite = presentation_prop_sprite(spec.id, state)
                    .unwrap_or_else(|| panic!("{} must have a canonical sprite", spec.id));
                assert!(
                    !sprite.is_empty(),
                    "{} must have a nonempty sprite",
                    spec.id
                );
                for cell in sprite {
                    assert!(
                        footprint.min_dx <= cell.dx && cell.dx <= footprint.max_dx,
                        "{} state {state:?} cell {cell:?} exceeds horizontal footprint {footprint:?}",
                        spec.id
                    );
                    assert!(
                        footprint.min_dy <= cell.dy && cell.dy <= footprint.max_dy,
                        "{} state {state:?} cell {cell:?} exceeds vertical footprint {footprint:?}",
                        spec.id
                    );
                }
            }
        }
    }

    #[test]
    fn vine_occupied_offsets_cover_every_animation_state() {
        assert_eq!(
            presentation_prop_occupied_offsets(TOKEN_HANGING_VINE_25M).unwrap(),
            vec![[0, 0], [0, 2], [1, 0], [1, 1], [1, 2], [2, 0], [2, 2]],
        );
    }
}
