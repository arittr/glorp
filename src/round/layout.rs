use crate::round::model::{RoundHelperHealth, RoundSceneModel, RoundSourceDiversity};

pub const SAFE_INNER_RADIUS_RATIO: f32 = 0.78;
const SAFE_INNER_RADIUS_MIN: f32 = 5.0;
const PET_RADIUS_RATIO: f32 = 0.58;
const PET_RADIUS_MIN: f32 = 7.0;
const PET_VERTICAL_OFFSET_RATIO: f32 = 0.10;
const PROP_HORIZONTAL_OFFSET_RATIO: f32 = 0.46;
const PROP_VERTICAL_OFFSET_RATIO: f32 = 0.48;
const PROP_RADIUS_RATIO: f32 = 0.16;
const DETAIL_FULL_THRESHOLD: f32 = 22.0;
const DETAIL_COMPACT_THRESHOLD: f32 = 13.0;
const HALO_CORNER_OFFSET_RATIO: f32 = 0.66;
const HALO_TOP_OFFSET: f32 = 1.0; // halo bead radius

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundAperture {
    pub width: u16,
    pub height: u16,
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
}

impl RoundAperture {
    pub fn new(width: u16, height: u16) -> Self {
        let radius = (width.min(height) as f32 / 2.0) - 1.0;
        Self {
            width,
            height,
            center_x: (width as f32 - 1.0) / 2.0,
            center_y: (height as f32 - 1.0) / 2.0,
            radius,
        }
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        let dx = x - self.center_x;
        let dy = y - self.center_y;
        dx * dx + dy * dy <= self.radius * self.radius
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoundSceneLayout {
    pub aperture: RoundAperture,
    pub safe_inner_radius: f32,
    pub detail_level: RoundDetailLevel,
    pub pet_anchor: RoundAnchor,
    pub prop_anchors: Vec<RoundAnchor>,
    pub halo_anchors: Vec<RoundAnchor>,
    pub motion_budget: RoundMotionBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundDetailLevel {
    Full,
    Compact,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundAnchor {
    pub kind: RoundAnchorKind,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundAnchorKind {
    Pet,
    Prop,
    ActivityPulse,
    SourceDiversity,
    Vital,
    HelperTrouble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundMotionBudget {
    pub pet_breath: bool,
    pub pet_blink: bool,
    pub activity_sweep: bool,
    pub prop_resonance: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundRenderCapabilities {
    pub truecolor: bool,
    pub transparent_outside_aperture: bool,
}

impl RoundRenderCapabilities {
    pub fn preview_truecolor() -> Self {
        Self {
            truecolor: true,
            transparent_outside_aperture: true,
        }
    }

    pub fn preview_flat() -> Self {
        Self {
            truecolor: false,
            transparent_outside_aperture: true,
        }
    }
}

/// V1 apertures are expected to be at least 28×28. The pet radius floor is
/// tuned for legibility at that size; smaller apertures may exceed the safe
/// inner circle and are not supported in V1.
///
/// `_capabilities` is reserved for future capability gating (e.g., flat color,
/// transparent background, high-DPI). V1 uses the same layout regardless.
pub fn layout_round_scene(
    scene: &RoundSceneModel,
    aperture: RoundAperture,
    _capabilities: RoundRenderCapabilities,
) -> RoundSceneLayout {
    let detail_level = if aperture.radius >= DETAIL_FULL_THRESHOLD {
        RoundDetailLevel::Full
    } else if aperture.radius >= DETAIL_COMPACT_THRESHOLD {
        RoundDetailLevel::Compact
    } else {
        RoundDetailLevel::Minimal
    };
    let safe_inner_radius = (aperture.radius * SAFE_INNER_RADIUS_RATIO).max(SAFE_INNER_RADIUS_MIN);
    let pet_anchor = RoundAnchor {
        kind: RoundAnchorKind::Pet,
        x: aperture.center_x,
        y: aperture.center_y + aperture.radius * PET_VERTICAL_OFFSET_RATIO,
        radius: (safe_inner_radius * PET_RADIUS_RATIO).max(PET_RADIUS_MIN),
    };
    let prop_limit = match detail_level {
        RoundDetailLevel::Full => 2,
        RoundDetailLevel::Compact => 1,
        RoundDetailLevel::Minimal => 0,
    };
    let prop_anchors: Vec<_> = scene
        .room
        .prop_landmarks
        .iter()
        .take(prop_limit)
        .enumerate()
        .map(|(index, _)| RoundAnchor {
            kind: RoundAnchorKind::Prop,
            x: aperture.center_x
                + if index == 0 {
                    -safe_inner_radius * PROP_HORIZONTAL_OFFSET_RATIO
                } else {
                    safe_inner_radius * PROP_HORIZONTAL_OFFSET_RATIO
                },
            y: aperture.center_y + safe_inner_radius * PROP_VERTICAL_OFFSET_RATIO,
            radius: safe_inner_radius * PROP_RADIUS_RATIO,
        })
        .collect();
    let has_prop_anchors = !prop_anchors.is_empty();
    let mut halo_anchors = Vec::new();
    if matches!(
        detail_level,
        RoundDetailLevel::Full | RoundDetailLevel::Compact
    ) {
        halo_anchors.push(RoundAnchor {
            kind: RoundAnchorKind::ActivityPulse,
            x: aperture.center_x,
            y: aperture.center_y - aperture.radius,
            radius: HALO_TOP_OFFSET,
        });
    }
    if matches!(detail_level, RoundDetailLevel::Full)
        && scene.halo.source_diversity != RoundSourceDiversity::Quiet
    {
        halo_anchors.push(RoundAnchor {
            kind: RoundAnchorKind::SourceDiversity,
            x: aperture.center_x + aperture.radius * HALO_CORNER_OFFSET_RATIO,
            y: aperture.center_y - aperture.radius * HALO_CORNER_OFFSET_RATIO,
            radius: HALO_TOP_OFFSET,
        });
        halo_anchors.push(RoundAnchor {
            kind: RoundAnchorKind::Vital,
            x: aperture.center_x,
            y: aperture.center_y + aperture.radius,
            radius: HALO_TOP_OFFSET,
        });
    }
    if scene.halo.helper_health == RoundHelperHealth::Trouble {
        halo_anchors.push(RoundAnchor {
            kind: RoundAnchorKind::HelperTrouble,
            x: aperture.center_x - aperture.radius * HALO_CORNER_OFFSET_RATIO,
            y: aperture.center_y - aperture.radius * HALO_CORNER_OFFSET_RATIO,
            radius: HALO_TOP_OFFSET,
        });
    }
    RoundSceneLayout {
        aperture,
        safe_inner_radius,
        detail_level,
        pet_anchor,
        prop_anchors,
        halo_anchors,
        motion_budget: RoundMotionBudget {
            pet_breath: true,
            pet_blink: true,
            activity_sweep: !scene.halo.activity_pulse.is_quiet(),
            prop_resonance: has_prop_anchors,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::round::model::{derive_round_scene_model, RoundHelperHealth};
    use crate::tui::view_model::WatchViewModel;
    use time::macros::datetime;

    #[test]
    fn layout_keeps_pet_inside_safe_inner_circle() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));
        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(52, 52),
            RoundRenderCapabilities::preview_truecolor(),
        );

        assert!(layout.safe_inner_radius > 18.0);
        assert!(layout
            .aperture
            .contains(layout.pet_anchor.x, layout.pet_anchor.y));
        assert!(layout.pet_anchor.radius <= layout.safe_inner_radius);
    }

    #[test]
    fn layout_drops_optional_halo_before_pet_legibility() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));
        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(28, 28),
            RoundRenderCapabilities::preview_truecolor(),
        );

        assert_eq!(layout.detail_level, RoundDetailLevel::Compact);
        assert!(layout.halo_anchors.len() <= 2);
        assert!(layout.pet_anchor.radius >= 7.0);
    }

    #[test]
    fn helper_trouble_keeps_one_visible_rim_anchor_in_compact_layout() {
        let mut vm = WatchViewModel::fixture();
        vm.source_health[0].status = crate::tui::view_model::SourceStatus::Diagnostic;
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));
        assert_eq!(scene.halo.helper_health, RoundHelperHealth::Trouble);

        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(28, 28),
            RoundRenderCapabilities::preview_truecolor(),
        );

        assert!(layout
            .halo_anchors
            .iter()
            .any(|anchor| anchor.kind == RoundAnchorKind::HelperTrouble));
    }

    #[test]
    fn layout_halo_and_prop_anchors_stay_inside_aperture() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));
        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(52, 52),
            RoundRenderCapabilities::preview_truecolor(),
        );

        for anchor in &layout.halo_anchors {
            assert!(
                layout.aperture.contains(anchor.x, anchor.y),
                "halo anchor outside aperture: {:?}",
                anchor
            );
        }
        for anchor in &layout.prop_anchors {
            assert!(
                layout.aperture.contains(anchor.x, anchor.y),
                "prop anchor outside aperture: {:?}",
                anchor
            );
        }
    }

    #[test]
    fn layout_uses_minimal_detail_for_tiny_aperture() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));
        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(10, 10),
            RoundRenderCapabilities::preview_truecolor(),
        );

        assert_eq!(layout.detail_level, RoundDetailLevel::Minimal);
        assert!(layout.prop_anchors.is_empty());
        assert!(layout.halo_anchors.is_empty());
    }

    #[test]
    fn layout_uses_smaller_dimension_for_non_square_aperture() {
        let vm = WatchViewModel::fixture_with_habitat_props();
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));
        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(80, 40),
            RoundRenderCapabilities::preview_truecolor(),
        );

        assert_eq!(layout.aperture.width, 80);
        assert_eq!(layout.aperture.height, 40);
        assert!(layout.aperture.radius <= 19.5);
        assert!(layout
            .aperture
            .contains(layout.pet_anchor.x, layout.pet_anchor.y));
    }
}
