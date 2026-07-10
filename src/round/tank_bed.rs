use crate::presentation::smooth::{
    CompanionViewport, SmoothBounds, SmoothFill, SmoothPoint, SmoothRgba8, SmoothShape,
    SmoothShapeGeometry,
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

/// The bed is a receding substrate seen through tank water, not a solid floor. One
/// hue, one monotone vertical falloff: strongest at the near floor, fading to
/// almost nothing at the horizon.
const BED_NEAR_ALPHA: u8 = 120;
const BED_HORIZON_ALPHA: u8 = 28;

/// How far the bed's fill is lifted toward white. The biome primaries are wall
/// colours, dark enough that an alpha-blended bed sinks into the water and gives
/// the pet's shadow nothing to sit on.
const BED_LIFT: f32 = 0.45;

/// Flecks are the bed's texture and sit over the faintest band, so they carry a
/// little more weight than the bands beneath them.
const BED_FLECK_PRIMARY_ALPHA: u8 = 88;
const BED_FLECK_SECONDARY_ALPHA: u8 = 80;

/// Core alpha of the pet's floor projection at the far and near planes. The rim
/// always fades to nothing, so these can run strong without a hard edge.
const PROJECTION_ALPHA_FAR: f32 = 165.0;
const PROJECTION_ALPHA_NEAR: f32 = 235.0;

/// The projection's travel band on the bed, as fractions of the bed's depth below
/// the horizon. The lower bed sits under the bottom vignette and the HUD reserve,
/// where a shadow cannot read, so the band is collapsed into the upper half.
const PROJECTION_BAND_FAR: f32 = 0.10;
const PROJECTION_BAND_NEAR: f32 = 0.45;

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
    // Deep enough below the aperture for a real receding curve, shallow enough
    // that most of the vertical falloff stays on screen and lights the floor.
    let base = SmoothBounds {
        min: SmoothPoint { x: -width * 0.08, y: horizon_y },
        max: SmoothPoint { x: width * 1.08, y: height * 1.15 },
    };
    let (primary, secondary) = bed_colors(biome);
    let floor = lift(primary, BED_LIFT);
    let mut shapes = vec![ellipse(
        base,
        SmoothFill::LinearGradientY {
            top: with_alpha(floor, BED_HORIZON_ALPHA),
            bottom: with_alpha(floor, BED_NEAR_ALPHA),
        },
    )];

    // A dense speckle texture over the gradient: it masks the banding an 8-bit
    // gradient shows on a dark field, and thinning it toward the horizon is
    // itself a depth cue. `depth` biases speckles toward the near floor and lets
    // nearer speckles run larger and stronger, like a substrate seen at an angle.
    let mut hash = tank_bed_hash(viewport, biome);
    let bed_height = height - horizon_y;
    for _ in 0..42 {
        let depth = hash_unit(&mut hash).powf(0.55);
        let x = width * (0.02 + 0.94 * hash_unit(&mut hash));
        let y = horizon_y + bed_height * (0.04 + 0.9 * depth);
        let size = 0.5 + depth * 0.9;
        let fleck_width = width * (0.008 + 0.02 * hash_unit(&mut hash)) * size;
        let fleck_height = height * (0.006 + 0.014 * hash_unit(&mut hash)) * size;
        let strength = 0.45 + 0.55 * depth;
        let color = if hash_unit(&mut hash) < 0.5 {
            with_alpha(
                lift(primary, 0.18),
                (f32::from(BED_FLECK_PRIMARY_ALPHA) * strength) as u8,
            )
        } else {
            with_alpha(
                lift(secondary, 0.18),
                (f32::from(BED_FLECK_SECONDARY_ALPHA) * strength) as u8,
            )
        };
        shapes.push(ellipse(
            SmoothBounds {
                min: SmoothPoint { x, y },
                max: SmoothPoint { x: x + fleck_width, y: y + fleck_height },
            },
            SmoothFill::Solid(color),
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
    let center_y = lerp(
        bed.horizon_y + PROJECTION_BAND_FAR * bed_height,
        bed.horizon_y + PROJECTION_BAND_NEAR * bed_height,
        t,
    );
    let radius_x = lerp(0.07 * width, 0.13 * width, t);
    let radius_y = lerp(0.016 * height, 0.04 * height, t);
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
    // A soft rim: a hard-edged solid blob reads as a hole in the bed, not a shadow.
    Some(ellipse(
        bounds,
        SmoothFill::RadialGradient {
            inner: with_alpha(bed.shadow, alpha),
            outer: with_alpha(bed.shadow, 0),
        },
    ))
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

/// The projection's multiply factor: darkens the floor beneath it to roughly
/// forty percent at the core, keeping the biome primary's hue so the shade reads
/// as the bed's own colour in shadow.
fn bed_shadow(primary: SmoothRgba8) -> SmoothRgba8 {
    SmoothRgba8 {
        r: primary.r / 3 + 30,
        g: primary.g / 3 + 30,
        b: primary.b / 3 + 30,
        a: 255,
    }
}

fn ellipse(bounds: SmoothBounds, fill: SmoothFill) -> SmoothShape {
    SmoothShape {
        geometry: SmoothShapeGeometry::Ellipse { bounds },
        fill,
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

fn lift(color: SmoothRgba8, amount: f32) -> SmoothRgba8 {
    let lift_channel =
        |channel: u8| (f32::from(channel) + (255.0 - f32::from(channel)) * amount).round() as u8;
    SmoothRgba8 {
        r: lift_channel(color.r),
        g: lift_channel(color.g),
        b: lift_channel(color.b),
        a: color.a,
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
