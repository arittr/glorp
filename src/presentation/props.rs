use crate::game::habitat::HabitatPetLayer;
use crate::presentation::target::SurfaceTargetId;
use crate::tui::component::habitat_props::{HabitatPropCell, HabitatPropPlacement};

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
