//! Renderer-neutral companion effect geometry and paint constants.
//!
//! Both companion presentation paths consume these pure contracts so depth
//! cues cannot drift or reverse the neutral scene dependency direction.

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WallShadowDepthCue {
    pub detach_cells: f32,
    pub strength: f32,
}

pub(crate) const PARALLAX_FAR_MULTIPLIER: f32 = 0.006;
pub(crate) const PARALLAX_MID_MULTIPLIER: f32 = 0.010;
pub(crate) const PARALLAX_BEHIND_MULTIPLIER: f32 = 0.014;
pub(crate) const PARALLAX_FOREGROUND_MULTIPLIER: f32 = 0.022;
pub(crate) const PARALLAX_MAX_X_CELLS: f32 = 0.25;
pub(crate) const PARALLAX_MAX_Y_CELLS: f32 = 0.15;

const WALL_SHADOW_DETACH_FAR: f32 = 0.45;
const WALL_SHADOW_DETACH_NEAR: f32 = 1.2;
const WALL_SHADOW_STRENGTH_FAR: f32 = 1.0;
const WALL_SHADOW_STRENGTH_NEAR: f32 = 0.0;

#[allow(dead_code)] // Consumed by the staged AppKit, Pixel, and retained integrations.
pub(crate) const PET_RIM_ENABLED: bool = true;
#[allow(dead_code)] // Consumed by the staged AppKit, Pixel, and retained integrations.
pub(crate) const PET_RIM_RADIUS_POINTS: f32 = 1.25;
#[allow(dead_code)] // Consumed by the staged AppKit, Pixel, and retained integrations.
pub(crate) const PET_RIM_IDLE_ALPHA: f32 = 0.09;
#[allow(dead_code)] // Consumed by the staged AppKit, Pixel, and retained integrations.
pub(crate) const PET_RIM_ACTIVITY_ALPHA_BONUS: f32 = 0.07;
#[allow(dead_code)] // Consumed by the staged AppKit, Pixel, and retained integrations.
const STATISTICS_REAR_SHADOW_LENGTH_CELLS: f32 = 0.90;
#[allow(dead_code)] // Consumed by the staged AppKit, Pixel, and retained integrations.
const STATISTICS_REAR_SHADOW_SOFTNESS_CELLS: f32 = 0.75;
#[allow(dead_code)] // Consumed by the staged AppKit, Pixel, and retained integrations.
const STATISTICS_REAR_SHADOW_OPACITY: f32 = 0.10;

#[allow(dead_code)] // Consumed by the staged AppKit, Pixel, and retained integrations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StatisticsRearShadowStyle {
    pub(crate) offset_y_up_points: [f32; 2],
    pub(crate) softness_points: f32,
    pub(crate) opacity: f32,
}

#[allow(dead_code)] // Consumed by the staged AppKit, Pixel, and retained integrations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PetRimStyle {
    pub(crate) enabled: bool,
    pub(crate) radius_points: f32,
    pub(crate) alpha: f32,
}

/// Resolves the fixed, rearward statistics shadow in physical point space.
///
/// The shared prop key-light direction is the only authored direction for
/// cast shadows, so each renderer receives the same backend-neutral vector.
#[allow(dead_code)] // Consumed by the staged AppKit, Pixel, and retained integrations.
pub(crate) fn statistics_rear_shadow_style(
    cell_extent_points: [f32; 2],
) -> Option<StatisticsRearShadowStyle> {
    if cell_extent_points
        .into_iter()
        .any(|extent| !extent.is_finite() || extent <= 0.0)
    {
        return None;
    }

    let vertical_extent_points = cell_extent_points[1];
    let length_points = STATISTICS_REAR_SHADOW_LENGTH_CELLS * vertical_extent_points;
    let softness_points = STATISTICS_REAR_SHADOW_SOFTNESS_CELLS * vertical_extent_points;
    let offset_y_up_points = crate::presentation::props::PROP_CAST_SHADOW_DIRECTION_Y_UP
        .map(|component| component * length_points);
    if offset_y_up_points
        .into_iter()
        .any(|value| !value.is_finite())
        || !softness_points.is_finite()
        || !STATISTICS_REAR_SHADOW_OPACITY.is_finite()
        || !(0.0..=0.12).contains(&STATISTICS_REAR_SHADOW_OPACITY)
    {
        return None;
    }

    Some(StatisticsRearShadowStyle {
        offset_y_up_points,
        softness_points,
        opacity: STATISTICS_REAR_SHADOW_OPACITY,
    })
}

/// Resolves the single optional pet rim. Activity may lift opacity, but never
/// changes its body-local radius; reduced motion keeps the idle opacity.
#[allow(dead_code)] // Consumed by the staged AppKit, Pixel, and retained integrations.
pub(crate) fn pet_rim_style(activity_opacity: f32, reduce_motion: bool) -> PetRimStyle {
    let disabled = PetRimStyle {
        enabled: false,
        radius_points: PET_RIM_RADIUS_POINTS,
        alpha: 0.0,
    };
    if !PET_RIM_ENABLED || !activity_opacity.is_finite() {
        return disabled;
    }

    let activity_opacity = activity_opacity.clamp(0.0, 1.0);
    let activity_bonus = if reduce_motion {
        0.0
    } else {
        activity_opacity * PET_RIM_ACTIVITY_ALPHA_BONUS
    };
    PetRimStyle {
        enabled: true,
        radius_points: PET_RIM_RADIUS_POINTS,
        alpha: PET_RIM_IDLE_ALPHA + activity_bonus,
    }
}

/// Returns an alpha-only rim mask, with source coverage subtracted so callers
/// can paint the pet above it without leaving an interior aura.
#[allow(dead_code)] // Consumed by the staged AppKit, Pixel, and retained integrations.
pub(crate) fn exterior_dilated_alpha(
    source: &[u8],
    width: u32,
    height: u32,
    radius_pixels: u32,
) -> Option<Vec<u8>> {
    if width == 0 || height == 0 {
        return None;
    }

    let expected_length = usize::try_from(u64::from(width) * u64::from(height)).ok()?;
    if source.len() != expected_length {
        return None;
    }

    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let radius = usize::try_from(radius_pixels).ok()?;
    let horizontal_radius = radius.min(width - 1);
    let vertical_radius = radius.min(height - 1);
    let mut exterior = vec![0; source.len()];

    for y in 0..height {
        let top = y.saturating_sub(vertical_radius);
        let bottom = y.saturating_add(vertical_radius).min(height - 1);
        for x in 0..width {
            let left = x.saturating_sub(horizontal_radius);
            let right = x.saturating_add(horizontal_radius).min(width - 1);
            let neighborhood_alpha = (top..=bottom)
                .flat_map(|neighbor_y| {
                    source[neighbor_y * width + left..=neighbor_y * width + right]
                        .iter()
                        .copied()
                })
                .max()
                .unwrap_or_default();
            let index = y * width + x;
            exterior[index] = neighborhood_alpha.saturating_sub(source[index]);
        }
    }

    Some(exterior)
}

pub(crate) const fn depth_lifecycle_scale(asleep: bool, _calm: bool) -> f32 {
    if asleep {
        0.25
    } else {
        1.0
    }
}

pub(crate) fn effective_depth(raw_z: f32, lifecycle_scale: f32) -> f32 {
    raw_z.clamp(-1.0, 1.0) * lifecycle_scale.clamp(0.0, 1.0)
}

pub(crate) fn wall_shadow_depth_cue(effective_z: f32) -> WallShadowDepthCue {
    let depth01 = ((effective_z + 1.0) * 0.5).clamp(0.0, 1.0);
    WallShadowDepthCue {
        detach_cells: lerp(WALL_SHADOW_DETACH_FAR, WALL_SHADOW_DETACH_NEAR, depth01),
        strength: lerp(WALL_SHADOW_STRENGTH_FAR, WALL_SHADOW_STRENGTH_NEAR, depth01),
    }
}

const PROJECTION_ALPHA_FAR: f32 = 165.0;
const PROJECTION_ALPHA_NEAR: f32 = 235.0;
const PROJECTION_BAND_FAR: f32 = 0.18;
const PROJECTION_BAND_NEAR: f32 = 0.80;
const PROJECTION_RADIUS_X_FAR: f32 = 0.085;
const PROJECTION_RADIUS_X_NEAR: f32 = 0.11;
const PROJECTION_RADIUS_Y_FAR: f32 = 0.022;
const PROJECTION_RADIUS_Y_NEAR: f32 = 0.032;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FloorProjectionMetrics {
    pub center_x: f32,
    pub center_y: f32,
    pub radius_x: f32,
    pub radius_y: f32,
    pub alpha: u8,
}

/// Canonical depth-only floor projection. Coordinates are Y-down in the
/// caller's logical units; Y-up consumers flip only the returned center Y.
pub(crate) fn floor_projection_metrics(
    width: f32,
    height: f32,
    horizon_y: f32,
    near_edge_y: f32,
    pet_center_x: f32,
    effective_z: f32,
) -> Option<FloorProjectionMetrics> {
    if !width.is_finite()
        || !height.is_finite()
        || !horizon_y.is_finite()
        || !near_edge_y.is_finite()
        || !pet_center_x.is_finite()
        || !effective_z.is_finite()
        || width <= 0.0
        || height <= 0.0
    {
        return None;
    }
    let bed_height = near_edge_y - horizon_y;
    if !bed_height.is_finite() || bed_height <= 0.0 {
        return None;
    }

    let depth01 = ((effective_z + 1.0) * 0.5).clamp(0.0, 1.0);
    let center_y = lerp(
        horizon_y + PROJECTION_BAND_FAR * bed_height,
        horizon_y + PROJECTION_BAND_NEAR * bed_height,
        depth01,
    );
    let radius_x = lerp(
        PROJECTION_RADIUS_X_FAR * width,
        PROJECTION_RADIUS_X_NEAR * width,
        depth01,
    );
    let radius_y = lerp(
        PROJECTION_RADIUS_Y_FAR * height,
        PROJECTION_RADIUS_Y_NEAR * height,
        depth01,
    );
    if !center_y.is_finite()
        || !radius_x.is_finite()
        || radius_x <= 0.0
        || !radius_y.is_finite()
        || radius_y <= 0.0
    {
        return None;
    }

    Some(FloorProjectionMetrics {
        center_x: pet_center_x.clamp(radius_x, (width - radius_x).max(radius_x)),
        center_y,
        radius_x,
        radius_y,
        alpha: lerp(PROJECTION_ALPHA_FAR, PROJECTION_ALPHA_NEAR, depth01)
            .round()
            .clamp(0.0, 255.0) as u8,
    })
}

pub(crate) fn bed_primary_srgb8(primary_biome: &str) -> [u8; 3] {
    match primary_biome {
        "botanical" => [63, 111, 102],
        "technical" => [70, 91, 125],
        "celestial" => [89, 75, 125],
        "artifact" => [108, 83, 102],
        "cozy" => [105, 78, 106],
        _ => [72, 83, 108],
    }
}

pub(crate) fn bed_shadow_srgb8(primary_biome: &str) -> [u8; 3] {
    bed_primary_srgb8(primary_biome).map(|channel| channel / 3 + 30)
}

pub(crate) fn bed_fleck_srgb8(primary_biome: &str) -> [u8; 3] {
    let primary = bed_primary_srgb8(primary_biome);
    let shadow = bed_shadow_srgb8(primary_biome);
    std::array::from_fn(|channel| {
        ((u16::from(primary[channel]) + 2 * u16::from(shadow[channel])) / 3) as u8
    })
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BedTextureSample {
    pub(crate) bed_mix: f32,
    pub(crate) broad_tone_levels: f32,
    pub(crate) grain_mix: f32,
    pub(crate) fleck_mix: f32,
    pub(crate) bed_srgb8: [u8; 3],
    pub(crate) fleck_srgb8: [u8; 3],
}

#[cfg(test)]
fn substrate_hash01(cell: [u32; 2], salt: u32) -> f32 {
    let mut hash = cell[0].wrapping_mul(0x9E37_79B9) ^ cell[1].wrapping_mul(0x85EB_CA6B) ^ salt;
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7FEB_352D);
    hash ^= hash >> 15;
    (hash & 0xFFFF) as f32 / 65535.0
}

#[cfg(test)]
fn texture_smooth_step(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
fn substrate_value_noise(point: [f32; 2], cell_size: f32, salt: u32) -> f32 {
    let grid = [point[0] / cell_size, point[1] / cell_size];
    let base = [grid[0].floor() as u32, grid[1].floor() as u32];
    let fraction = [grid[0].fract(), grid[1].fract()];
    let weight = [
        fraction[0] * fraction[0] * (3.0 - 2.0 * fraction[0]),
        fraction[1] * fraction[1] * (3.0 - 2.0 * fraction[1]),
    ];
    let n00 = substrate_hash01(base, salt);
    let n10 = substrate_hash01([base[0] + 1, base[1]], salt);
    let n01 = substrate_hash01([base[0], base[1] + 1], salt);
    let n11 = substrate_hash01([base[0] + 1, base[1] + 1], salt);
    let top = n00 + (n10 - n00) * weight[0];
    let bottom = n01 + (n11 - n01) * weight[0];
    top + (bottom - top) * weight[1]
}

#[cfg(test)]
fn substrate_mark(point: [f32; 2], cell_size: f32, radius: f32, density: f32, salt: u32) -> f32 {
    let grid = [point[0] / cell_size, point[1] / cell_size];
    let cell = [grid[0].floor() as u32, grid[1].floor() as u32];
    if substrate_hash01(cell, salt) >= density {
        return 0.0;
    }
    let center_span = (cell_size - 2.0 * radius).max(0.0);
    let center = [
        radius + substrate_hash01(cell, salt ^ 0xA511_E9B3) * center_span,
        radius + substrate_hash01(cell, salt ^ 0x63D8_35A7) * center_span,
    ];
    let local = [grid[0].fract() * cell_size, grid[1].fract() * cell_size];
    let distance = ((local[0] - center[0]).powi(2) + (local[1] - center[1]).powi(2)).sqrt();
    1.0 - texture_smooth_step(radius - 0.75, radius + 0.75, distance)
}

#[cfg(test)]
fn room_bed_mix(logical_point_y_down: [f32; 2], logical_extent: [f32; 2]) -> f32 {
    let normalized_x = logical_point_y_down[0] / logical_extent[0] - 0.5;
    let horizon_y = logical_extent[1] * (0.76 + 0.04 * normalized_x * normalized_x);
    let bed_feather = (logical_extent[1] * 0.12).max(1.0);
    let bed_t = ((logical_point_y_down[1] - horizon_y) / bed_feather).clamp(0.0, 1.0);
    bed_t * bed_t * (3.0 - 2.0 * bed_t)
}

#[cfg(test)]
pub(crate) fn bed_texture_sample(
    logical_point_y_down: [f32; 2],
    logical_extent: [f32; 2],
    backing_scale: f32,
    primary_biome: &str,
) -> BedTextureSample {
    let bed_mix = room_bed_mix(logical_point_y_down, logical_extent);

    let texture_gate = texture_smooth_step(0.18, 0.82, bed_mix);
    let broad_tone_levels = (substrate_value_noise(logical_point_y_down, 36.0, 0xC13F_A9A9) - 0.5)
        * 14.0
        * texture_gate;
    let grain_mix =
        substrate_mark(logical_point_y_down, 10.0, 1.6, 0.50, 0x91E1_0DA5) * 0.22 * texture_gate;
    let fleck_mix =
        substrate_mark(logical_point_y_down, 30.0, 2.8, 0.28, 0xD1B5_4A35) * 0.48 * texture_gate;
    let _ = backing_scale;

    BedTextureSample {
        bed_mix,
        broad_tone_levels,
        grain_mix,
        fleck_mix,
        bed_srgb8: bed_primary_srgb8(primary_biome),
        fleck_srgb8: bed_fleck_srgb8(primary_biome),
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WallTextureSample {
    pub(crate) bed_mix: f32,
    pub(crate) wall_gate: f32,
    pub(crate) broad_tone_levels: f32,
    pub(crate) strata_levels: f32,
    pub(crate) mineral_levels: f32,
}

#[cfg(test)]
pub(crate) fn wall_texture_sample(
    logical_point_y_down: [f32; 2],
    logical_extent: [f32; 2],
    backing_scale: f32,
) -> WallTextureSample {
    let bed_mix = room_bed_mix(logical_point_y_down, logical_extent);
    let wall_gate = 1.0 - texture_smooth_step(0.02, 0.68, bed_mix);
    let rock_point = [logical_point_y_down[0] * 0.55, logical_point_y_down[1]];
    let rock_field = substrate_value_noise(rock_point, 54.0, 0x4A11_5EED);
    let broad_tone_levels = (rock_field - 0.5) * 10.0 * wall_gate;
    let strata_phase = (logical_point_y_down[1] + (rock_field - 0.5) * 24.0) / 38.0;
    let strata_fraction = strata_phase - strata_phase.floor();
    let strata_distance = (strata_fraction - 0.5).abs();
    let strata_levels = (1.0 - texture_smooth_step(0.025, 0.10, strata_distance)) * 4.0 * wall_gate;
    let mineral_levels =
        substrate_mark(logical_point_y_down, 24.0, 1.4, 0.18, 0x6D2B_79F5) * 4.0 * wall_gate;
    let _ = backing_scale;

    WallTextureSample {
        bed_mix,
        wall_gate,
        broad_tone_levels,
        strata_levels,
        mineral_levels,
    }
}

pub(crate) const TANK_DEPTH_TINT_SRGB: [f32; 3] = [0.10, 0.11, 0.20];
pub(crate) const TANK_CORE_TINT_WEIGHT: f32 = 0.42;
pub(crate) const TANK_WALL_AMBIENT_LIFT_SRGB: [f32; 3] = [0.050, 0.055, 0.070];

pub(crate) fn biome_background_srgb(primary_biome: &str) -> [f32; 3] {
    match primary_biome {
        "botanical" => [0.07, 0.11, 0.08],
        "technical" => [0.07, 0.09, 0.13],
        "celestial" => [0.08, 0.08, 0.14],
        "artifact" => [0.12, 0.10, 0.07],
        "cozy" => [0.13, 0.09, 0.08],
        _ => [0.08, 0.09, 0.10],
    }
}

pub(crate) fn phase_dim_background_srgb(primary_biome: &str, phase_scale: f32) -> [f32; 3] {
    biome_background_srgb(primary_biome).map(|channel| channel * phase_scale)
}

pub(crate) fn tank_core_srgb(background: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|index| {
        background[index]
            + (TANK_DEPTH_TINT_SRGB[index] - background[index]) * TANK_CORE_TINT_WEIGHT
    })
}

pub(crate) fn tank_background_paint_srgb8(
    primary_biome: &str,
    phase_scale: f32,
) -> ([u8; 3], [u8; 3]) {
    let dimmed = phase_dim_background_srgb(primary_biome, phase_scale);
    let rim = std::array::from_fn(|channel| {
        (dimmed[channel] + TANK_WALL_AMBIENT_LIFT_SRGB[channel]).clamp(0.0, 1.0)
    });
    (srgb8(tank_core_srgb(rim)), srgb8(rim))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GaugeLaneLayout {
    pub radius: f64,
    pub stroke_width: f64,
    pub track_start_degrees: f64,
    pub track_sweep_degrees: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PerimeterGaugeLayout {
    pub xp: GaugeLaneLayout,
    pub daily: GaugeLaneLayout,
    pub pace: GaugeLaneLayout,
}

pub(crate) const COMPANION_GAUGE_GAP_DEGREES: f64 = 70.0;

pub(crate) fn perimeter_gauge_layout(
    aperture_radius: f64,
    gap_degrees: f64,
) -> PerimeterGaugeLayout {
    let gap = gap_degrees.clamp(0.0, 180.0);
    let track_start_degrees = 270.0 + gap / 2.0;
    let track_sweep_degrees = 360.0 - gap;
    let outer_inset = 3.0_f64.max(aperture_radius * 0.012);
    let xp_width = (aperture_radius * 0.050).clamp(6.0, 16.0);
    let daily_width = (aperture_radius * 0.040).clamp(5.0, 13.0);
    let pace_width = (aperture_radius * 0.034).clamp(4.0, 11.0);
    let lane_gap = (aperture_radius * 0.010).clamp(1.5, 4.0);
    let xp_radius = aperture_radius - outer_inset - xp_width / 2.0;
    let daily_radius = xp_radius - xp_width / 2.0 - lane_gap - daily_width / 2.0;
    let pace_radius = daily_radius - daily_width / 2.0 - lane_gap - pace_width / 2.0;
    let lane = |radius, stroke_width| GaugeLaneLayout {
        radius,
        stroke_width,
        track_start_degrees,
        track_sweep_degrees,
    };
    PerimeterGaugeLayout {
        xp: lane(xp_radius, xp_width),
        daily: lane(daily_radius, daily_width),
        pace: lane(pace_radius, pace_width),
    }
}

pub(crate) const GAUGE_XP_TRACK_SRGBA: [f32; 4] = [0.71, 0.71, 0.78, 0.16];
pub(crate) const GAUGE_XP_FILL_SRGBA: [f32; 4] = [0.61, 0.48, 0.88, 0.90];
pub(crate) const GAUGE_DAILY_TRACK_SRGBA: [f32; 4] = [0.22, 0.36, 0.23, 0.12];
pub(crate) const GAUGE_DAILY_FILL_SRGBA: [f32; 4] = [0.24, 0.43, 0.24, 0.78];
pub(crate) const GAUGE_PACE_TRACK_SRGBA: [f32; 4] = [0.96, 0.68, 0.31, 0.13];
pub(crate) const GAUGE_PACE_FILL_SRGBA: [f32; 4] = [0.98, 0.67, 0.27, 0.86];
pub(crate) const GAUGE_DAILY_OVERAGE_SRGBA: [f32; 4] = [0.36, 0.60, 0.30, 0.95];
pub(crate) const GAUGE_DAILY_ROLLOVER_CAP_SRGB: [f32; 3] = [0.86, 0.98, 0.56];
pub(crate) const GAUGE_DAILY_ROLLOVER_REMAINING_FACTOR: f32 = 0.55;
pub(crate) const STATUS_CALM_SRGBA: [f32; 4] = [0.36, 0.40, 0.55, 0.80];
pub(crate) const STATUS_ACTIVE_SRGBA: [f32; 4] = [0.94, 0.65, 0.28, 0.90];
pub(crate) const TROUBLE_SRGBA: [f32; 4] = [0.92, 0.30, 0.25, 0.95];
pub(crate) const RETAINED_WALL_SHADOW_TINT_SRGB8: [u8; 3] = [10, 12, 18];
pub(crate) const SMOOTH_WALL_SHADOW_MULTIPLY_SRGB8: [u8; 3] = [118, 114, 142];
pub(crate) const RETAINED_WALL_SHADOW_TINT_ALPHA_U8: u8 = 78;
pub(crate) const MOOD_AURA_RING_ALPHA_U8: u8 = 13;
pub(crate) const MOOD_CONTENT_SRGBA: [f32; 4] = [0.25, 0.71, 0.60, 1.0];
pub(crate) const MOOD_HAPPY_SRGBA: [f32; 4] = [0.82, 0.45, 0.62, 1.0];
pub(crate) const MOOD_ECSTATIC_SRGBA: [f32; 4] = [0.95, 0.40, 0.70, 1.0];
pub(crate) const MOOD_HUNGRY_SRGBA: [f32; 4] = [0.85, 0.62, 0.30, 1.0];
pub(crate) const MOOD_SAD_SRGBA: [f32; 4] = [0.40, 0.50, 0.78, 1.0];
pub(crate) const MOOD_SLEEPY_SRGBA: [f32; 4] = [0.55, 0.50, 0.80, 1.0];
pub(crate) const MOOD_WILTED_SRGBA: [f32; 4] = [0.45, 0.40, 0.48, 1.0];

pub(crate) fn daily_rollover_srgba(rollover: u32) -> [f32; 4] {
    let remaining = GAUGE_DAILY_ROLLOVER_REMAINING_FACTOR.powf(rollover.saturating_sub(1) as f32);
    let mut color = GAUGE_DAILY_OVERAGE_SRGBA;
    for (channel, cap) in color[..3].iter_mut().zip(GAUGE_DAILY_ROLLOVER_CAP_SRGB) {
        *channel = cap + (*channel - cap) * remaining;
    }
    color
}

/// Packed shader contract: cap RGB followed by the per-rollover amount of
/// color distance that remains, all as unsigned normalized bytes.
pub(crate) fn daily_rollover_contract_unorm8() -> [u8; 4] {
    srgba8([
        GAUGE_DAILY_ROLLOVER_CAP_SRGB[0],
        GAUGE_DAILY_ROLLOVER_CAP_SRGB[1],
        GAUGE_DAILY_ROLLOVER_CAP_SRGB[2],
        GAUGE_DAILY_ROLLOVER_REMAINING_FACTOR,
    ])
}

pub(crate) fn srgba8(color: [f32; 4]) -> [u8; 4] {
    color.map(|channel| (channel * 255.0).round().clamp(0.0, 255.0) as u8)
}

pub(crate) fn srgb8(color: [f32; 3]) -> [u8; 3] {
    color.map(|channel| (channel * 255.0).round().clamp(0.0, 255.0) as u8)
}

pub(crate) fn mood_aura_radius(transformed_pet_width: f64) -> f64 {
    transformed_pet_width.max(0.0) * 0.95
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1.0e-5,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn wall_shadow_depth_cue_disappears_at_front_glass() {
        let far_wall = wall_shadow_depth_cue(-1.0);
        let near_wall = wall_shadow_depth_cue(1.0);
        assert_close(far_wall.detach_cells, 0.45);
        assert_close(near_wall.detach_cells, 1.2);
        assert_close(far_wall.strength, 1.0);
        assert_close(near_wall.strength, 0.0);
    }

    #[test]
    fn floor_projection_traverses_from_rear_to_near_lip() {
        let far_floor = floor_projection_metrics(100.0, 100.0, 20.0, 80.0, 50.0, -1.0)
            .expect("valid far floor projection");
        let near_floor = floor_projection_metrics(100.0, 100.0, 20.0, 80.0, 50.0, 1.0)
            .expect("valid near floor projection");
        assert_close(far_floor.center_y, 30.8);
        assert_close(near_floor.center_y, 68.0);
        assert_close(far_floor.radius_x, 8.5);
        assert_close(near_floor.radius_x, 11.0);
        assert_close(far_floor.radius_y, 2.2);
        assert_close(near_floor.radius_y, 3.2);
        assert_eq!(far_floor.alpha, 165);
        assert_eq!(near_floor.alpha, 235);
    }

    #[test]
    fn tank_wall_palette_stays_dark_readable_and_phase_ordered() {
        let luma = |rgb: [u8; 3]| {
            0.2126 * f32::from(rgb[0]) + 0.7152 * f32::from(rgb[1]) + 0.0722 * f32::from(rgb[2])
        };
        let mut day_palettes = Vec::new();
        for biome in [
            "starter",
            "botanical",
            "technical",
            "celestial",
            "artifact",
            "cozy",
        ] {
            let phases =
                [0.60, 0.80, 0.85, 1.00].map(|scale| tank_background_paint_srgb8(biome, scale));
            for pair in phases.windows(2) {
                assert!(
                    luma(pair[0].1) < luma(pair[1].1),
                    "{biome} phase ordering: {phases:?}",
                );
            }

            let (night_core, night_rim) = phases[0];
            let (day_core, day_rim) = phases[3];
            assert!(night_rim
                .into_iter()
                .all(|channel| (23..=39).contains(&channel)));
            assert!(night_core
                .into_iter()
                .all(|channel| (24..=44).contains(&channel)));
            assert!(day_rim
                .into_iter()
                .all(|channel| (31..=54).contains(&channel)));
            assert!(day_core
                .into_iter()
                .all(|channel| (28..=52).contains(&channel)));
            assert!(luma(day_rim) < luma(bed_primary_srgb8(biome)));
            assert!(
                !day_palettes.contains(&(day_core, day_rim)),
                "{biome} keeps a distinct tank palette",
            );
            day_palettes.push((day_core, day_rim));
        }

        assert_eq!(
            tank_background_paint_srgb8("starter", 1.0),
            ([30, 33, 47], [33, 37, 43])
        );
        assert_eq!(
            tank_background_paint_srgb8("starter", 0.6),
            ([25, 28, 41], [25, 28, 33])
        );
    }

    #[test]
    fn bed_texture_biome_palette_is_deterministic_visible_and_bounded() {
        for (biome, primary, shadow, fleck) in [
            ("starter", [72, 83, 108], [54, 57, 66], [60, 65, 80]),
            ("botanical", [63, 111, 102], [51, 67, 64], [55, 81, 76]),
            ("technical", [70, 91, 125], [53, 60, 71], [58, 70, 89]),
            ("celestial", [89, 75, 125], [59, 55, 71], [69, 61, 89]),
            ("artifact", [108, 83, 102], [66, 57, 64], [80, 65, 76]),
            ("cozy", [105, 78, 106], [65, 56, 65], [78, 63, 78]),
        ] {
            assert_eq!(bed_primary_srgb8(biome), primary, "{biome} primary");
            assert_eq!(bed_shadow_srgb8(biome), shadow, "{biome} shadow");
            assert_eq!(bed_fleck_srgb8(biome), fleck, "{biome} fleck");
            for channel in 0..3 {
                assert!(
                    shadow[channel] <= fleck[channel],
                    "{biome} channel {channel}"
                );
                assert!(
                    fleck[channel] <= primary[channel],
                    "{biome} channel {channel}"
                );
            }
            assert!(
                primary
                    .iter()
                    .zip(fleck)
                    .any(|(primary, fleck)| primary.abs_diff(fleck) >= 12),
                "{biome} fleck must survive room blending"
            );
        }
    }

    #[test]
    fn bed_texture_is_invariant_to_backing_scale() {
        let at_1x = bed_texture_sample([144.5, 300.5], [360.0; 2], 1.0, "starter");
        let at_2x = bed_texture_sample([144.5, 300.5], [360.0; 2], 2.0, "starter");
        assert_eq!(at_1x, at_2x);
        assert!(at_1x.bed_mix > 0.6);
    }

    #[test]
    fn bed_texture_changes_with_biome() {
        let starter = bed_texture_sample([144.5, 300.5], [360.0; 2], 1.0, "starter");
        let botanical = bed_texture_sample([144.5, 300.5], [360.0; 2], 1.0, "botanical");
        assert_eq!(starter.bed_mix, botanical.bed_mix);
        assert_eq!(starter.broad_tone_levels, botanical.broad_tone_levels);
        assert_eq!(starter.grain_mix, botanical.grain_mix);
        assert_eq!(starter.fleck_mix, botanical.fleck_mix);
        assert_eq!(starter.bed_srgb8, [72, 83, 108]);
        assert_eq!(starter.fleck_srgb8, [60, 65, 80]);
        assert_eq!(botanical.bed_srgb8, [63, 111, 102]);
        assert_eq!(botanical.fleck_srgb8, [55, 81, 76]);
    }

    #[test]
    fn bed_texture_has_broad_tone_and_clustered_marks() {
        let samples = (276..=348)
            .step_by(2)
            .flat_map(|y| {
                (48..=312).step_by(2).map(move |x| {
                    bed_texture_sample([x as f32 + 0.5, y as f32 + 0.5], [360.0; 2], 1.0, "starter")
                })
            })
            .collect::<Vec<_>>();
        assert!(samples
            .iter()
            .any(|sample| sample.broad_tone_levels.abs() >= 2.0));
        assert!(samples.iter().any(|sample| sample.grain_mix >= 0.10));
        assert!(samples.iter().any(|sample| sample.fleck_mix >= 0.20));
    }

    #[test]
    fn wall_texture_is_invariant_to_backing_scale() {
        let at_1x = wall_texture_sample([144.5, 120.5], [360.0; 2], 1.0);
        let at_2x = wall_texture_sample([144.5, 120.5], [360.0; 2], 2.0);
        assert_eq!(at_1x, at_2x);
        assert_eq!(at_1x.bed_mix, 0.0);
        assert_eq!(at_1x.wall_gate, 1.0);
    }

    #[test]
    fn wall_texture_has_broad_strata_and_sparse_mineral_structure() {
        let samples = (36..=240)
            .step_by(2)
            .flat_map(|y| {
                (36..=324).step_by(2).map(move |x| {
                    wall_texture_sample([x as f32 + 0.5, y as f32 + 0.5], [360.0; 2], 1.0)
                })
            })
            .collect::<Vec<_>>();
        assert!(samples
            .iter()
            .any(|sample| sample.broad_tone_levels.abs() >= 2.0));
        assert!(samples.iter().any(|sample| sample.strata_levels >= 2.5));
        assert!(samples.iter().any(|sample| sample.mineral_levels >= 2.0));
        assert!(samples.iter().all(|sample| {
            (sample.broad_tone_levels - sample.strata_levels + sample.mineral_levels).abs() <= 14.0
        }));
    }

    #[test]
    fn wall_texture_fades_out_before_ground_substrate() {
        for y in (320..=356).step_by(4) {
            for x in (48..=312).step_by(12) {
                let sample = wall_texture_sample([x as f32 + 0.5, y as f32 + 0.5], [360.0; 2], 2.0);
                assert_eq!(sample.wall_gate, 0.0, "sample=({x}, {y})");
                assert_eq!(sample.broad_tone_levels, 0.0);
                assert_eq!(sample.strata_levels, 0.0);
                assert_eq!(sample.mineral_levels, 0.0);
            }
        }
    }

    #[test]
    fn upper_room_samples_never_emit_flecks() {
        for y in (24..=252).step_by(12) {
            for x in (48..=312).step_by(12) {
                let sample =
                    bed_texture_sample([x as f32 + 0.5, y as f32 + 0.5], [360.0; 2], 2.0, "cozy");
                assert_eq!(sample.bed_mix, 0.0, "sample=({x}, {y})");
                assert_eq!(sample.broad_tone_levels, 0.0, "sample=({x}, {y})");
                assert_eq!(sample.grain_mix, 0.0, "sample=({x}, {y})");
                assert_eq!(sample.fleck_mix, 0.0, "sample=({x}, {y})");
            }
        }
    }

    fn assert_parallel(actual: [f32; 2], expected: [f32; 2]) {
        let cross_product = actual[0] * expected[1] - actual[1] * expected[0];
        assert!(
            cross_product.abs() < 1.0e-5,
            "expected {actual:?} to be parallel with {expected:?}",
        );
    }

    fn center_index(width: u32, height: u32) -> usize {
        (height as usize / 2) * width as usize + width as usize / 2
    }

    fn neighbor_index(width: u32, height: u32) -> usize {
        center_index(width, height) + 1
    }

    fn alpha_fixture_with_one_opaque_center_pixel() -> Vec<u8> {
        let mut source = vec![0; 9];
        source[center_index(3, 3)] = u8::MAX;
        source
    }

    #[test]
    fn statistics_shadow_uses_prop_key_light_and_stays_soft_and_faint() {
        let style = statistics_rear_shadow_style([7.0, 14.0]).unwrap();
        assert!(style.offset_y_up_points[0] > 0.0);
        assert!(style.offset_y_up_points[1] < 0.0);
        assert!(style.softness_points >= 7.0);
        assert!(style.opacity > 0.0 && style.opacity <= 0.12);
        assert_parallel(
            style.offset_y_up_points,
            crate::presentation::props::PROP_CAST_SHADOW_DIRECTION_Y_UP,
        );
    }

    #[test]
    fn statistics_shadow_rejects_invalid_cell_extents_without_non_finite_output() {
        for extent in [
            [0.0, 14.0],
            [7.0, 0.0],
            [-7.0, 14.0],
            [7.0, -14.0],
            [f32::NAN, 14.0],
            [7.0, f32::INFINITY],
        ] {
            assert_eq!(statistics_rear_shadow_style(extent), None, "{extent:?}");
        }

        let style = statistics_rear_shadow_style([f32::MAX, f32::MAX]).unwrap();
        assert!(style.offset_y_up_points.into_iter().all(f32::is_finite));
        assert!(style.softness_points.is_finite());
        assert!(style.opacity.is_finite());
        assert!(style.opacity > 0.0 && style.opacity <= 0.12);
    }

    #[test]
    fn pet_rim_is_narrow_constant_backed_and_activity_only_changes_alpha() {
        let idle = pet_rim_style(0.0, false);
        let active = pet_rim_style(1.0, false);
        let reduced = pet_rim_style(1.0, true);
        let clamped_low = pet_rim_style(-1.0, false);
        let clamped_high = pet_rim_style(2.0, false);
        assert!(idle.enabled);
        assert!(idle.radius_points > 0.0 && idle.radius_points <= 1.5);
        assert!(idle.alpha > 0.0 && idle.alpha <= 0.12);
        assert_eq!(active.radius_points, idle.radius_points);
        assert!(active.alpha > idle.alpha);
        assert_eq!(reduced.alpha, idle.alpha);
        assert_eq!(clamped_low.alpha, idle.alpha);
        assert_eq!(clamped_high.alpha, active.alpha);
    }

    #[test]
    fn mask_dilation_returns_only_exterior_coverage() {
        let source = alpha_fixture_with_one_opaque_center_pixel();
        let rim = exterior_dilated_alpha(&source, 3, 3, 1).unwrap();
        assert_eq!(rim[center_index(3, 3)], 0);
        assert!(rim[neighbor_index(3, 3)] > 0);
    }

    #[test]
    fn mask_dilation_rejects_invalid_dimensions_and_subtracts_source_alpha() {
        assert!(exterior_dilated_alpha(&[], 0, 0, 1).is_none());
        assert!(exterior_dilated_alpha(&[u8::MAX], 2, 2, 1).is_none());

        let source = vec![32, 128, 255, 64];
        let rim = exterior_dilated_alpha(&source, 2, 2, 1).unwrap();
        assert_eq!(rim, vec![223, 127, 0, 191]);
    }

    #[test]
    fn invalid_rim_inputs_disable_the_rim_without_restoring_an_aura() {
        let invalid = pet_rim_style(f32::NAN, false);
        assert!(!invalid.enabled);
        assert_eq!(invalid.alpha, 0.0);
    }
}
