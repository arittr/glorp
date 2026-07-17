use super::art_reference::{PixelArtReferenceProvider, PixelArtRole, PixelPetArtReference};
use super::frame::{PixelFrame, PixelViewport, Rgba8};
use super::input::{PixelPetInput, PixelVariationKey};
use super::raster::{alpha_blend_pixel, fill_circle, fill_ellipse, fill_rect};
use super::scene::PixelPetScene;
use crate::pet::generation::Species;

#[derive(Debug, Clone)]
pub struct PixelRendererState {
    pub(crate) start: time::OffsetDateTime,
    art_reference_provider: PixelArtReferenceProvider,
}

impl PixelRendererState {
    pub fn new(_input: &PixelPetInput, now: time::OffsetDateTime) -> Self {
        Self {
            start: now,
            art_reference_provider: PixelArtReferenceProvider::default(),
        }
    }

    pub fn art_reference_for(
        &mut self,
        request: &super::art_reference::PixelArtReferenceRequest,
    ) -> PixelPetArtReference {
        self.art_reference_provider.reference_for(request)
    }
}

pub struct PixelRendererTick<'a> {
    pub input: &'a PixelPetInput,
    pub art_reference: &'a PixelPetArtReference,
    pub viewport: PixelViewport,
    pub now: time::OffsetDateTime,
    pub state: &'a mut PixelRendererState,
}

pub fn render_pixel_frame(tick: PixelRendererTick<'_>) -> PixelFrame {
    let scene = PixelPetScene::from_input_and_reference(
        tick.input,
        tick.art_reference,
        tick.state,
        tick.now,
    );
    let mut frame = PixelFrame::transparent(tick.viewport);
    let cx =
        i16::try_from(tick.viewport.logical_width / 2).unwrap() + scene.wander_x.round() as i16;
    let cy =
        i16::try_from(tick.viewport.logical_height / 2).unwrap() + scene.breath_y.round() as i16;
    draw_shadow(&mut frame, cx, cy + scene.body_ry + 6, scene.body_rx);
    if tick.art_reference.occupied_cells.is_empty() {
        draw_fallback_body(&mut frame, tick.input, &scene, cx, cy);
        draw_fallback_face(&mut frame, tick.input, &scene, cx, cy);
        draw_fallback_accents(&mut frame, tick.input, &scene, cx, cy);
    } else {
        draw_reference_cells(&mut frame, tick.input, &scene, tick.art_reference, cx, cy);
        if tick.art_reference.role_count(PixelArtRole::Eye) == 0 {
            draw_fallback_face(&mut frame, tick.input, &scene, cx, cy);
        }
    }
    clear_outside_round_aperture(&mut frame);
    frame
}

fn draw_shadow(frame: &mut PixelFrame, cx: i16, cy: i16, body_rx: i16) {
    let shadow = Rgba8 { r: 8, g: 12, b: 16, a: 88 };
    blend_ellipse(frame, cx, cy, body_rx + 4, 6, shadow);
}

fn draw_reference_cells(
    frame: &mut PixelFrame,
    input: &PixelPetInput,
    scene: &PixelPetScene,
    reference: &PixelPetArtReference,
    cx: i16,
    cy: i16,
) {
    for cell in &reference.occupied_cells {
        let color = color_for_role(input, cell.role);
        let x = cx + scene.reference_origin_x + i16::from(cell.x) * scene.reference_scale;
        let y = cy + scene.reference_origin_y + i16::from(cell.y) * scene.reference_scale;
        fill_rect(
            frame,
            x,
            y,
            scene.reference_scale,
            scene.reference_scale,
            color,
        );
    }
}

fn draw_fallback_body(
    frame: &mut PixelFrame,
    input: &PixelPetInput,
    scene: &PixelPetScene,
    cx: i16,
    cy: i16,
) {
    let body = rgba_opaque(input.palette.body);
    match input.identity.species {
        Species::Fuzz | Species::Blob | Species::Ghost => {
            fill_ellipse(frame, cx, cy, scene.body_rx, scene.body_ry, body);
            if scene.wispy {
                let haze = rgba_with_alpha(input.palette.body, 92);
                blend_ellipse(
                    frame,
                    cx,
                    cy - 4,
                    scene.body_rx - 2,
                    scene.body_ry + 3,
                    haze,
                );
                blend_ellipse(
                    frame,
                    cx,
                    cy + 6,
                    scene.body_rx - 6,
                    scene.body_ry - 2,
                    haze,
                );
            }
        }
        Species::Glitch => {
            fill_rect(
                frame,
                cx - scene.body_rx,
                cy - scene.body_ry + 4,
                scene.body_rx * 2,
                scene.body_ry * 2 - 8,
                body,
            );
            fill_rect(
                frame,
                cx - scene.body_rx + 4,
                cy - scene.body_ry,
                scene.body_rx * 2 - 8,
                7,
                body,
            );
            fill_rect(
                frame,
                cx - scene.body_rx + 3,
                cy + scene.body_ry - 6,
                scene.body_rx * 2 - 6,
                6,
                body,
            );
        }
        Species::Crystal => {
            fill_rect(
                frame,
                cx - 4,
                cy - scene.body_ry,
                8,
                scene.body_ry * 2,
                body,
            );
            fill_rect(
                frame,
                cx - scene.body_rx + 4,
                cy - scene.body_ry + 7,
                scene.body_rx * 2 - 8,
                scene.body_ry * 2 - 14,
                body,
            );
            fill_rect(
                frame,
                cx - scene.body_rx + 8,
                cy - scene.body_ry + 2,
                scene.body_rx * 2 - 16,
                scene.body_ry * 2 - 4,
                body,
            );
        }
        Species::Mech => {
            fill_rect(
                frame,
                cx - scene.body_rx,
                cy - scene.body_ry + 2,
                scene.body_rx * 2,
                scene.body_ry * 2 - 4,
                body,
            );
            fill_rect(
                frame,
                cx - scene.body_rx + 5,
                cy - scene.body_ry - 3,
                scene.body_rx * 2 - 10,
                7,
                body,
            );
            fill_rect(
                frame,
                cx - scene.body_rx + 3,
                cy + scene.body_ry - 5,
                scene.body_rx * 2 - 6,
                5,
                body,
            );
            fill_circle(frame, cx, cy, 4, rgba_opaque(input.palette.pattern));
        }
    }
}

fn draw_fallback_face(
    frame: &mut PixelFrame,
    input: &PixelPetInput,
    scene: &PixelPetScene,
    cx: i16,
    cy: i16,
) {
    let eye = rgba_opaque(input.palette.eye);
    if scene.blink_closed {
        fill_rect(frame, cx - 11, cy - 2, 5, 1, eye);
        fill_rect(frame, cx + 6, cy - 2, 5, 1, eye);
        return;
    }
    if input.sleep.asleep {
        let (left_x, right_x, width) = asleep_eye_spans(input.identity.species);
        fill_rect(frame, cx + left_x, cy + 2, width, 1, eye);
        fill_rect(frame, cx + right_x, cy + 2, width, 1, eye);
        return;
    }
    fill_rect(frame, cx - 10, cy - 4, 3, 4, eye);
    fill_rect(frame, cx + 7, cy - 4, 3, 4, eye);
}

fn draw_fallback_accents(
    frame: &mut PixelFrame,
    input: &PixelPetInput,
    scene: &PixelPetScene,
    cx: i16,
    cy: i16,
) {
    let accent = rgba_opaque(input.palette.accent);
    let glitch = rgba_opaque(input.palette.corruption);
    for idx in 0..usize::from(scene.accent_count) {
        let x_seed = jitter(
            input.identity.variation_key,
            idx as u16 * 7 + 1,
            scene.body_rx * 2 + 6,
        );
        let y_seed = jitter(
            input.identity.variation_key,
            idx as u16 * 7 + 2,
            scene.body_ry * 2 + 6,
        );
        let x = cx - scene.body_rx - 3 + x_seed;
        let y = cy - scene.body_ry - 3 + y_seed;
        match input.identity.species {
            Species::Crystal => fill_rect(frame, x, y, 2, 4, accent),
            Species::Mech => fill_rect(frame, x, y, 3, 2, accent),
            Species::Ghost => {
                blend_circle(frame, x, y, 2, rgba_with_alpha(input.palette.accent, 148))
            }
            _ => fill_rect(frame, x, y, 2, 2, accent),
        }
    }

    if input.identity.species == Species::Glitch
        && input.identity.stage == crate::game::evolution::Stage::S4
    {
        for idx in 0..5_usize {
            let x = cx - scene.body_rx - 2
                + jitter(
                    input.identity.variation_key,
                    80 + idx as u16 * 11,
                    scene.body_rx * 2 + 4,
                );
            let y = cy - scene.body_ry - 1
                + jitter(
                    input.identity.variation_key,
                    81 + idx as u16 * 11,
                    scene.body_ry * 2 + 2,
                );
            fill_rect(frame, x, y, 3, 2, glitch);
        }
    }
}

fn clear_outside_round_aperture(frame: &mut PixelFrame) {
    let radius = i16::try_from(frame.width.min(frame.height) / 2).unwrap();
    let cx = i16::try_from(frame.width / 2).unwrap();
    let cy = i16::try_from(frame.height / 2).unwrap();
    let radius_sq = i32::from(radius) * i32::from(radius);

    for y in 0..frame.height {
        for x in 0..frame.width {
            let dx = i32::from(x) - i32::from(cx);
            let dy = i32::from(y) - i32::from(cy);
            if dx * dx + dy * dy > radius_sq {
                let idx = usize::from(y) * usize::from(frame.width) + usize::from(x);
                frame.pixels[idx] = Rgba8::TRANSPARENT;
            }
        }
    }
}

fn jitter(key: PixelVariationKey, salt: u16, span: i16) -> i16 {
    let span = span.max(1);
    let bucket = (key.0.wrapping_add(salt.wrapping_mul(97))) % (span as u16);
    i16::try_from(bucket).unwrap()
}

fn blend_ellipse(frame: &mut PixelFrame, cx: i16, cy: i16, rx: i16, ry: i16, color: Rgba8) {
    let rx2 = i32::from(rx.max(1)).pow(2);
    let ry2 = i32::from(ry.max(1)).pow(2);
    let limit = rx2 * ry2;
    for y in cy - ry..=cy + ry {
        for x in cx - rx..=cx + rx {
            let dx = i32::from(x - cx);
            let dy = i32::from(y - cy);
            if dx * dx * ry2 + dy * dy * rx2 <= limit {
                alpha_blend_pixel(frame, x, y, color);
            }
        }
    }
}

fn blend_circle(frame: &mut PixelFrame, cx: i16, cy: i16, radius: i16, color: Rgba8) {
    blend_ellipse(frame, cx, cy, radius, radius, color);
}

fn rgba_opaque(rgb: crate::pet::palette::Rgb) -> Rgba8 {
    Rgba8::opaque(rgb.r, rgb.g, rgb.b)
}

fn rgba_with_alpha(rgb: crate::pet::palette::Rgb, alpha: u8) -> Rgba8 {
    Rgba8 { r: rgb.r, g: rgb.g, b: rgb.b, a: alpha }
}

fn rgba_scaled(rgb: crate::pet::palette::Rgb, scale: f32) -> Rgba8 {
    let scale = scale.clamp(0.0, 1.0);
    Rgba8::opaque(
        (f32::from(rgb.r) * scale).round() as u8,
        (f32::from(rgb.g) * scale).round() as u8,
        (f32::from(rgb.b) * scale).round() as u8,
    )
}

fn color_for_role(input: &PixelPetInput, role: PixelArtRole) -> Rgba8 {
    match role {
        PixelArtRole::Eye | PixelArtRole::Mouth => rgba_opaque(input.palette.eye),
        PixelArtRole::Corruption => rgba_opaque(input.palette.corruption),
        PixelArtRole::Pattern => rgba_opaque(input.palette.pattern),
        PixelArtRole::Accent | PixelArtRole::Particle => rgba_opaque(input.palette.accent),
        PixelArtRole::Locket | PixelArtRole::Facet | PixelArtRole::RepairMark => {
            rgba_opaque(input.palette.accent)
        }
        PixelArtRole::Outline => rgba_scaled(input.palette.body, 0.62),
        PixelArtRole::InteriorTexture => rgba_scaled(input.palette.body, 0.84),
        PixelArtRole::Appendage => rgba_scaled(input.palette.body, 0.92),
        PixelArtRole::FootContact => rgba_scaled(input.palette.body, 0.72),
        PixelArtRole::Body | PixelArtRole::BodyGlow => rgba_opaque(input.palette.body),
    }
}

fn asleep_eye_spans(species: Species) -> (i16, i16, i16) {
    match species {
        Species::Crystal => (-9, 5, 4),
        _ => (-10, 6, 4),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::pixel::art_reference::{
        PixelArtPoseKey, PixelCellBounds, PixelFootContact, PixelReferenceChecksum,
    };
    use crate::tui::view_model::WatchViewModel;
    use std::collections::BTreeMap;
    use time::macros::datetime;

    fn empty_reference(input: &PixelPetInput) -> PixelPetArtReference {
        PixelPetArtReference {
            species: input.identity.species,
            stage: input.identity.stage,
            mood: input.mood,
            pose: PixelArtPoseKey {
                tick: 0,
                hold_eyes_closed: false,
                blink_suppression_ticks: 0,
                blink_slowdown: 0,
                soft_eyes: false,
                work_accent: "none",
                feed_reaction: false,
                glitch_patch_tier: None,
                glitch_burst_level: None,
                glitch_day_key: None,
                glitch_calm_mode: false,
                glitch_feed_reaction: false,
            },
            width_cells: 0,
            height_cells: 0,
            occupied_cells: Vec::new(),
            body_bounds: PixelCellBounds { min_x: 0, min_y: 0, max_x: 0, max_y: 0 },
            foot_contact: PixelFootContact { cells: Vec::new() },
            protected_regions: Vec::new(),
            cue_coverage: BTreeMap::new(),
            reference_checksum: PixelReferenceChecksum(0),
            role_counts: BTreeMap::new(),
        }
    }

    #[test]
    fn pixel_idle_frame_has_no_large_ellipse_outside_the_body_shadow_and_rim_band() {
        let now = datetime!(2026-07-08 12:00 UTC);
        let input = PixelPetInput::from_watch_view_model(&WatchViewModel::fixture(), now);
        let reference = empty_reference(&input);
        let mut state = PixelRendererState::new(&input, now);
        let scene = PixelPetScene::from_input_and_reference(&input, &reference, &state, now);
        let frame = render_pixel_frame(PixelRendererTick {
            input: &input,
            art_reference: &reference,
            viewport: PixelViewport::companion_default(),
            now,
            state: &mut state,
        });
        let center_x = i16::try_from(frame.width / 2).unwrap() + scene.wander_x.round() as i16;
        let center_y = i16::try_from(frame.height / 2).unwrap() + scene.breath_y.round() as i16;
        let probe_x = center_x + scene.body_rx + 8;
        let probe_y = center_y;
        let probe = frame.pixels
            [usize::from(probe_y as u16) * usize::from(frame.width) + usize::from(probe_x as u16)];

        assert_eq!(
            probe,
            Rgba8::TRANSPARENT,
            "a broad idle ellipse leaked outside the body, shadow, and future narrow rim band"
        );
    }
}
