use crate::presentation::pixel::{PixelBounds, PixelViewport};
use crate::round::hud::{
    perimeter_gauge_layout, stat_gap_box, CompanionHudText, COMPANION_GAUGE_GAP_DEG,
};
use crate::round::layout::RoundAperture;

const APERTURE_MARGIN_RATIO: f32 = 0.08;
const HUD_ZONE_PADDING_RATIO: f32 = 0.035;
const BODY_BOTTOM_RATIO: f32 = 0.71;
const BODY_CLEARANCE_PX: f32 = 6.0;
const MAX_HUD_HEIGHT_RATIO: f32 = 0.34;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelTargetGeometry {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelFitRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PixelFitRect {
    pub fn overlaps(self, other: Self) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PixelCompanionFit {
    pub producer: &'static str,
    pub geometry: PixelTargetGeometry,
    pub aperture: RoundAperture,
    pub image_rect: PixelFitRect,
    pub creature_zone: PixelFitRect,
    pub hud_safe_zone: PixelFitRect,
    pub scale: f32,
}

pub fn pixel_companion_fit(
    geometry: PixelTargetGeometry,
    viewport: PixelViewport,
    hud: &CompanionHudText,
) -> PixelCompanionFit {
    let aperture = RoundAperture::new(geometry.width, geometry.height);
    let layout = perimeter_gauge_layout(
        f64::from(aperture.center_x),
        f64::from(aperture.center_y),
        f64::from(aperture.radius),
        COMPANION_GAUGE_GAP_DEG,
    );
    let gap = stat_gap_box(
        f64::from(aperture.center_x),
        f64::from(aperture.center_y),
        layout.pace.ring.radius - layout.pace.stroke_width / 2.0,
        COMPANION_GAUGE_GAP_DEG,
    );

    let companion_font_size = estimate_companion_font_size(geometry);
    let (hud_width, hud_height) = estimate_hud_stack(hud, companion_font_size);
    let hud_padding = (aperture.radius * HUD_ZONE_PADDING_RATIO).max(8.0);
    let hud_height_cap = aperture.radius * MAX_HUD_HEIGHT_RATIO;
    let hud_block_height = hud_height.min(hud_height_cap);
    let hud_safe_zone = PixelFitRect {
        x: (gap.center_x as f32 - (hud_width / 2.0 + hud_padding))
            .clamp(0.0, f32::from(geometry.width)),
        y: (gap.baseline_y as f32 - hud_block_height - hud_padding)
            .clamp(0.0, f32::from(geometry.height)),
        width: (hud_width + hud_padding * 2.0).min(f32::from(geometry.width)),
        height: (hud_block_height + hud_padding * 2.0).min(
            f32::from(geometry.height)
                - (gap.baseline_y as f32 - hud_block_height - hud_padding).max(0.0),
        ),
    };

    let margin = (aperture.radius * APERTURE_MARGIN_RATIO).max(8.0);
    let creature_zone = PixelFitRect {
        x: margin,
        y: margin,
        width: (f32::from(geometry.width) - margin * 2.0).max(0.0),
        height: (hud_safe_zone.y - margin * 2.0).max(0.0),
    };
    let logical_side = f32::from(viewport.logical_width.max(viewport.logical_height));
    let side_limit_for_body = if BODY_BOTTOM_RATIO > 0.0 {
        ((hud_safe_zone.y - margin - BODY_CLEARANCE_PX) / BODY_BOTTOM_RATIO).max(0.0)
    } else {
        0.0
    };
    let side = creature_zone
        .width
        .min(creature_zone.height)
        .min(side_limit_for_body)
        .max(logical_side.min(creature_zone.width.min(creature_zone.height)));
    let scale = if logical_side > 0.0 {
        side / logical_side
    } else {
        0.0
    };
    let centered_x = creature_zone.x + (creature_zone.width - side) / 2.0;
    let centered_y = creature_zone.y + (creature_zone.height - side) / 2.0;
    let body_lift_limit = hud_safe_zone.y - BODY_CLEARANCE_PX - side * BODY_BOTTOM_RATIO;
    let image_rect = PixelFitRect {
        x: centered_x.max(creature_zone.x),
        y: centered_y.min(body_lift_limit).max(creature_zone.y),
        width: side,
        height: side,
    };

    PixelCompanionFit {
        producer: "round::pixel_fit::pixel_companion_fit",
        geometry,
        aperture,
        image_rect,
        creature_zone,
        hud_safe_zone,
        scale,
    }
}

impl PixelCompanionFit {
    pub fn logical_bounds_overlap_hud(&self, bounds: PixelBounds) -> bool {
        let mapped = self.map_logical_bounds(bounds);
        mapped.overlaps(self.hud_safe_zone)
    }

    pub fn map_logical_bounds(&self, bounds: PixelBounds) -> PixelFitRect {
        let sx = self.image_rect.width / 96.0;
        let sy = self.image_rect.height / 96.0;
        PixelFitRect {
            x: self.image_rect.x + f32::from(bounds.min_x) * sx,
            y: self.image_rect.y + f32::from(bounds.min_y) * sy,
            width: f32::from(bounds.max_x - bounds.min_x + 1) * sx,
            height: f32::from(bounds.max_y - bounds.min_y + 1) * sy,
        }
    }
}

fn estimate_companion_font_size(geometry: PixelTargetGeometry) -> f32 {
    (f32::from(geometry.width.min(geometry.height)) / 22.0).clamp(10.0, 28.0)
}

fn estimate_hud_stack(hud: &CompanionHudText, companion_font_size: f32) -> (f32, f32) {
    let big_width = estimate_line_width(&hud.today_total, companion_font_size * 1.08);
    let daily_width = estimate_line_width(&hud.daily_percent, companion_font_size * 0.68);
    let pace_width = estimate_line_width(&hud.pace, companion_font_size * 0.68);
    let width = big_width.max(daily_width).max(pace_width);

    let big_height = companion_font_size * 1.08 * 1.12;
    let sub_height = companion_font_size * 0.68 * 1.12;
    let height = big_height + sub_height * 0.82 + sub_height * 0.82;

    (width, height)
}

fn estimate_line_width(text: &str, font_size: f32) -> f32 {
    text.chars().count() as f32 * font_size * 0.72
}
