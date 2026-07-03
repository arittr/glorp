use crate::pet::render::{PaletteRoleName, StyledSegment};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetTextBlock {
    pub lines: Vec<String>,
    pub spans: Vec<StyledSegment>,
}

impl PetTextBlock {
    pub fn new(lines: Vec<String>, spans: Vec<StyledSegment>) -> Self {
        Self { lines, spans }
    }
}

pub fn role_for_cell(block: &PetTextBlock, row: usize, char_index: usize) -> PaletteRoleName {
    block
        .spans
        .iter()
        .find(|span| span.line == row && char_index >= span.start && char_index < span.end)
        .map(|span| span.role)
        .unwrap_or(PaletteRoleName::Body)
}

pub fn role_names(spans: &[StyledSegment]) -> Vec<String> {
    let mut roles = spans
        .iter()
        .map(|span| role_name(span.role).to_string())
        .collect::<Vec<_>>();
    roles.sort();
    roles.dedup();
    roles
}

pub fn role_name(role: PaletteRoleName) -> &'static str {
    match role {
        PaletteRoleName::Body => "body",
        PaletteRoleName::BodyGlow => "body_glow",
        PaletteRoleName::Eye => "eye",
        PaletteRoleName::Mouth => "mouth",
        PaletteRoleName::Accent => "accent",
        PaletteRoleName::Pattern => "pattern",
        PaletteRoleName::Particle => "particle",
        PaletteRoleName::Corruption => "corruption",
    }
}
