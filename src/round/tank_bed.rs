use crate::presentation::smooth::{
    CompanionViewport, SmoothBounds, SmoothPoint, SmoothRgba8, SmoothShape, SmoothShapeGeometry,
};
use crate::tui::room::{RoomBiome, RoomBiomeTag};

#[derive(Debug, Clone, PartialEq)]
pub struct SmoothTankBedGeometry {
    pub shapes: Vec<SmoothShape>,
    pub horizon_y: f32,
    pub near_edge_y: f32,
}

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
        ellipse(base, with_alpha(primary, 192)),
        ellipse(
            inset_bounds(base, width * 0.06, height * 0.055),
            with_alpha(secondary, 126),
        ),
        ellipse(
            inset_bounds(base, width * 0.14, height * 0.125),
            with_alpha(primary, 92),
        ),
    ];

    let mut hash = tank_bed_hash(viewport, biome);
    for _ in 0..10 {
        let x = width * (0.03 + 0.9 * hash_unit(&mut hash));
        let y = horizon_y + height * (0.045 + 0.18 * hash_unit(&mut hash));
        let fleck_width = width * (0.012 + 0.028 * hash_unit(&mut hash));
        let fleck_height = height * (0.009 + 0.022 * hash_unit(&mut hash));
        let color = if hash_unit(&mut hash) < 0.5 {
            with_alpha(primary, 100)
        } else {
            with_alpha(secondary, 92)
        };
        shapes.push(ellipse(
            SmoothBounds {
                min: SmoothPoint { x, y },
                max: SmoothPoint { x: x + fleck_width, y: y + fleck_height },
            },
            color,
        ));
    }

    Some(SmoothTankBedGeometry { shapes, horizon_y, near_edge_y: height })
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
