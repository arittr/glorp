use crate::presentation::smooth::{
    CompanionViewport, SmoothBounds, SmoothPoint, SmoothRgba8, SmoothShape, SmoothShapeGeometry,
};
use crate::round::depth::SmoothDepthSample;
use crate::tui::room::{RoomBiome, RoomBiomeTag};

#[derive(Debug, Clone, PartialEq)]
pub struct SmoothTankBedGeometry {
    pub shapes: Vec<SmoothShape>,
    pub horizon_y: f32,
    pub near_edge_y: f32,
    /// Opaque base colour for anything cast onto the bed. Callers set their own
    /// alpha; the projection fades with depth.
    pub shadow: SmoothRgba8,
}

/// The bed is a receding substrate seen through tank water, not a solid floor.
/// These bands stack, so each stays translucent enough for the tank's darkness to
/// read through; opaque bands turn the bed into a footer bowl at companion size.
const BED_BASE_ALPHA: u8 = 70;
const BED_MID_ALPHA: u8 = 48;
const BED_INNER_ALPHA: u8 = 34;

/// Flecks are the bed's texture and sit over the faintest band, so they carry a
/// little more weight than the bands beneath them.
const BED_FLECK_PRIMARY_ALPHA: u8 = 60;
const BED_FLECK_SECONDARY_ALPHA: u8 = 55;

/// Alpha of the pet's floor projection at the far and near planes.
const PROJECTION_ALPHA_FAR: f32 = 46.0;
const PROJECTION_ALPHA_NEAR: f32 = 92.0;

/// Fraction of the bed's depth the projection keeps clear of each edge, so the
/// pet never appears to stand on the horizon line or off the near lip.
const PROJECTION_EDGE_INSET: f32 = 0.10;

pub fn smooth_tank_bed_geometry(
    viewport: CompanionViewport,
    biome: RoomBiome,
) -> Option<SmoothTankBedGeometry> {
    if viewport.grid_cols < 2 || viewport.grid_rows < 2 {
        return None;
    }

    let width = f32::from(viewport.grid_cols);
    let height = f32::from(viewport.grid_rows);
    let horizon_y = height * 0.76;
    let base = SmoothBounds {
        min: SmoothPoint { x: -width * 0.08, y: horizon_y },
        max: SmoothPoint { x: width * 1.08, y: height * 1.34 },
    };
    let (primary, secondary) = bed_colors(biome);
    let mut shapes = vec![
        ellipse(base, with_alpha(primary, BED_BASE_ALPHA)),
        ellipse(
            inset_bounds(base, width * 0.06, height * 0.055),
            with_alpha(secondary, BED_MID_ALPHA),
        ),
        ellipse(
            inset_bounds(base, width * 0.14, height * 0.125),
            with_alpha(primary, BED_INNER_ALPHA),
        ),
    ];

    let mut hash = tank_bed_hash(viewport, biome);
    for _ in 0..10 {
        let x = width * (0.03 + 0.9 * hash_unit(&mut hash));
        let y = horizon_y + height * (0.045 + 0.18 * hash_unit(&mut hash));
        let fleck_width = width * (0.012 + 0.028 * hash_unit(&mut hash));
        let fleck_height = height * (0.009 + 0.022 * hash_unit(&mut hash));
        let color = if hash_unit(&mut hash) < 0.5 {
            with_alpha(primary, BED_FLECK_PRIMARY_ALPHA)
        } else {
            with_alpha(secondary, BED_FLECK_SECONDARY_ALPHA)
        };
        shapes.push(ellipse(
            SmoothBounds {
                min: SmoothPoint { x, y },
                max: SmoothPoint { x: x + fleck_width, y: y + fleck_height },
            },
            color,
        ));
    }

    Some(SmoothTankBedGeometry {
        shapes,
        horizon_y,
        near_edge_y: height,
        shadow: bed_shadow(primary),
    })
}

/// The pet's contact projection on the bed, resolved from the same depth sample
/// that drives its scale and perspective. Near is larger, stronger, and further
/// down the bed; far is smaller, fainter, and closer to the horizon.
pub fn smooth_floor_projection_shape(
    viewport: CompanionViewport,
    bed: &SmoothTankBedGeometry,
    pet_center_x: f32,
    depth: SmoothDepthSample,
) -> Option<SmoothShape> {
    if viewport.grid_cols < 2 || viewport.grid_rows < 2 || !pet_center_x.is_finite() {
        return None;
    }
    let width = f32::from(viewport.grid_cols);
    let height = f32::from(viewport.grid_rows);
    let bed_height = bed.near_edge_y - bed.horizon_y;
    if !bed_height.is_finite() || bed_height <= 0.0 {
        return None;
    }

    let t = (depth.effective_z + 1.0) * 0.5;
    let inset = PROJECTION_EDGE_INSET * bed_height;
    let center_y = lerp(bed.horizon_y + inset, bed.near_edge_y - inset, t);
    let radius_x = lerp(0.055 * width, 0.105 * width, t);
    let radius_y = lerp(0.012 * height, 0.030 * height, t);
    if !radius_x.is_finite() || radius_x <= 0.0 || !radius_y.is_finite() || radius_y <= 0.0 {
        return None;
    }
    if !center_y.is_finite() {
        return None;
    }

    // Keep the whole ellipse inside the aperture's horizontal span.
    let center_x = pet_center_x.clamp(radius_x, (width - radius_x).max(radius_x));
    let alpha = lerp(PROJECTION_ALPHA_FAR, PROJECTION_ALPHA_NEAR, t)
        .round()
        .clamp(0.0, 255.0) as u8;

    let bounds = SmoothBounds {
        min: SmoothPoint {
            x: center_x - radius_x,
            y: center_y - radius_y,
        },
        max: SmoothPoint {
            x: center_x + radius_x,
            y: center_y + radius_y,
        },
    };
    if !bounds_are_finite(bounds) {
        return None;
    }
    Some(ellipse(bounds, with_alpha(bed.shadow, alpha)))
}

fn bounds_are_finite(bounds: SmoothBounds) -> bool {
    bounds.min.x.is_finite()
        && bounds.min.y.is_finite()
        && bounds.max.x.is_finite()
        && bounds.max.y.is_finite()
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// A cast shadow reads as the bed's own colour in shade, so darken the biome
/// primary rather than introducing an unrelated neutral.
fn bed_shadow(primary: SmoothRgba8) -> SmoothRgba8 {
    SmoothRgba8 {
        r: primary.r / 3,
        g: primary.g / 3,
        b: primary.b / 3,
        a: 255,
    }
}

fn ellipse(bounds: SmoothBounds, color: SmoothRgba8) -> SmoothShape {
    SmoothShape {
        geometry: SmoothShapeGeometry::Ellipse { bounds },
        color,
    }
}

fn inset_bounds(bounds: SmoothBounds, inset_x: f32, inset_y: f32) -> SmoothBounds {
    SmoothBounds {
        min: SmoothPoint {
            x: bounds.min.x + inset_x,
            y: bounds.min.y + inset_y,
        },
        max: SmoothPoint {
            x: bounds.max.x - inset_x,
            y: bounds.max.y - inset_y,
        },
    }
}

fn bed_colors(biome: RoomBiome) -> (SmoothRgba8, SmoothRgba8) {
    let primary = color_for_biome(biome.primary);
    let secondary = biome.secondary.map(color_for_biome).unwrap_or(SmoothRgba8 {
        r: 62,
        g: 116,
        b: 118,
        a: 255,
    });
    (primary, secondary)
}

fn color_for_biome(biome: RoomBiomeTag) -> SmoothRgba8 {
    match biome {
        RoomBiomeTag::Starter => SmoothRgba8 { r: 72, g: 83, b: 108, a: 255 },
        RoomBiomeTag::Botanical => SmoothRgba8 { r: 63, g: 111, b: 102, a: 255 },
        RoomBiomeTag::Technical => SmoothRgba8 { r: 70, g: 91, b: 125, a: 255 },
        RoomBiomeTag::Celestial => SmoothRgba8 { r: 89, g: 75, b: 125, a: 255 },
        RoomBiomeTag::Artifact => SmoothRgba8 { r: 108, g: 83, b: 102, a: 255 },
        RoomBiomeTag::Cozy => SmoothRgba8 { r: 105, g: 78, b: 106, a: 255 },
    }
}

fn with_alpha(color: SmoothRgba8, a: u8) -> SmoothRgba8 {
    SmoothRgba8 { a, ..color }
}

fn tank_bed_hash(viewport: CompanionViewport, biome: RoomBiome) -> u32 {
    let mut hash = 0x9E37_79B9;
    for value in [
        biome_tag_hash(biome.primary),
        biome.secondary.map(biome_tag_hash).unwrap_or(0),
        u32::from(viewport.grid_cols),
        u32::from(viewport.grid_rows),
    ] {
        hash ^= value
            .wrapping_add(0x9E37_79B9)
            .wrapping_add(hash << 6)
            .wrapping_add(hash >> 2);
    }
    hash
}

fn biome_tag_hash(tag: RoomBiomeTag) -> u32 {
    match tag {
        RoomBiomeTag::Starter => 1,
        RoomBiomeTag::Botanical => 2,
        RoomBiomeTag::Technical => 3,
        RoomBiomeTag::Celestial => 4,
        RoomBiomeTag::Artifact => 5,
        RoomBiomeTag::Cozy => 6,
    }
}

fn hash_unit(hash: &mut u32) -> f32 {
    *hash ^= *hash << 13;
    *hash ^= *hash >> 17;
    *hash ^= *hash << 5;
    (*hash as f32) / (u32::MAX as f32)
}
