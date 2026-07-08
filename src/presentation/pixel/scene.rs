use super::animator::PixelRendererState;
use super::art_reference::PixelPetArtReference;
use super::input::PixelPetInput;
use crate::pet::generation::Species;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelPetScene {
    pub body_rx: i16,
    pub body_ry: i16,
    pub accent_count: u8,
    pub blocky: bool,
    pub wispy: bool,
    pub wander_x: f32,
    pub breath_y: f32,
    pub blink_closed: bool,
    pub pulse_alpha: f32,
    pub reference_scale: i16,
    pub reference_origin_x: i16,
    pub reference_origin_y: i16,
}

impl PixelPetScene {
    pub fn from_input_and_reference(
        input: &PixelPetInput,
        art_reference: &PixelPetArtReference,
        state: &PixelRendererState,
        now: time::OffsetDateTime,
    ) -> Self {
        let elapsed_ms = i64::try_from((now - state.start).whole_milliseconds()).unwrap_or(
            if now >= state.start {
                i64::MAX
            } else {
                i64::MIN
            },
        );
        Self::from_elapsed_ms_and_reference(input, art_reference, elapsed_ms)
    }

    pub(crate) fn from_elapsed_ms(input: &PixelPetInput, elapsed_ms: i64) -> Self {
        Self::from_elapsed_ms_and_reference(input, &fallback_reference(input), elapsed_ms)
    }

    fn from_elapsed_ms_and_reference(
        input: &PixelPetInput,
        art_reference: &PixelPetArtReference,
        elapsed_ms: i64,
    ) -> Self {
        let stage_scale = 1.0 + input.identity.stage.index() as f32 * 0.075;
        let calm_mult = if input.sleep.calm { 0.28 } else { 1.0 };
        let wander_phase =
            elapsed_ms as f32 / 900.0 + f32::from(input.identity.variation_key.bucket(19)) * 0.17;
        let breath_phase = elapsed_ms as f32 / if input.sleep.asleep { 1_900.0 } else { 760.0 };
        let blink_period = 2_600 + i64::from(input.identity.variation_key.bucket(900));
        let blink_closed = elapsed_ms.rem_euclid(blink_period) < 120 && !input.sleep.asleep;
        let pulse_alpha = if input.pulse.active {
            (1.0 - (f32::from(input.pulse.age_ms) / 2_000.0)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let (base_rx, base_ry, accent_count, blocky, wispy) = match input.identity.species {
            Species::Fuzz => (15, 13, 4, false, false),
            Species::Blob => (16, 11, 2, false, false),
            Species::Ghost => (13, 16, 3, false, true),
            Species::Glitch => (14, 12, 7, true, false),
            Species::Crystal => (13, 15, 6, true, false),
            Species::Mech => (15, 12, 5, true, false),
        };

        let fallback_body_rx = (base_rx as f32 * stage_scale).round() as i16;
        let fallback_body_ry = (base_ry as f32 * stage_scale).round() as i16;
        let (body_rx, body_ry, reference_scale, reference_origin_x, reference_origin_y) =
            reference_geometry(art_reference, fallback_body_rx, fallback_body_ry);

        Self {
            body_rx,
            body_ry,
            accent_count,
            blocky,
            wispy,
            wander_x: wander_phase.sin() * 7.0 * calm_mult,
            breath_y: breath_phase.sin() * if input.sleep.asleep { 1.0 } else { 2.4 },
            blink_closed,
            pulse_alpha,
            reference_scale,
            reference_origin_x,
            reference_origin_y,
        }
    }
}

fn reference_geometry(
    art_reference: &PixelPetArtReference,
    fallback_body_rx: i16,
    fallback_body_ry: i16,
) -> (i16, i16, i16, i16, i16) {
    if art_reference.occupied_cells.is_empty() {
        return (fallback_body_rx, fallback_body_ry, 0, 0, 0);
    }

    let bounds = art_reference.body_bounds;
    let reference_scale = ((fallback_body_rx * 2) / i16::from(bounds.width()).max(1)).max(1);
    let body_width = i16::from(bounds.width()) * reference_scale;
    let body_height = i16::from(bounds.height()) * reference_scale;
    let body_rx = (body_width / 2).max(1);
    let body_ry = (body_height / 2).max(1);
    let reference_origin_x =
        -(i16::from(bounds.min_x) * reference_scale) - ((body_width - reference_scale) / 2);
    let reference_origin_y =
        -(i16::from(bounds.min_y) * reference_scale) - ((body_height - reference_scale) / 2);

    (
        body_rx,
        body_ry,
        reference_scale,
        reference_origin_x,
        reference_origin_y,
    )
}

fn fallback_reference(input: &PixelPetInput) -> PixelPetArtReference {
    PixelPetArtReference {
        species: input.identity.species,
        stage: input.identity.stage,
        mood: input.mood,
        pose: super::art_reference::PixelArtPoseKey {
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
        body_bounds: super::art_reference::PixelCellBounds {
            min_x: 0,
            min_y: 0,
            max_x: 0,
            max_y: 0,
        },
        foot_contact: super::art_reference::PixelFootContact { cells: Vec::new() },
        protected_regions: Vec::new(),
        cue_coverage: std::collections::BTreeMap::new(),
        reference_checksum: super::art_reference::PixelReferenceChecksum(0),
        role_counts: std::collections::BTreeMap::new(),
    }
}
