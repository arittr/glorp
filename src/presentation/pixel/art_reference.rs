use crate::game::{evolution::Stage, metabolism::Mood};
use crate::pet::generation::{generate_pet, Species};
use crate::pet::render::{AnimationFrame, PaletteRoleName, WorkAccent};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

const LOCKET_GLYPHS: [char; 3] = ['◌', '◆', '◈'];
const FACET_GLYPHS: [char; 4] = ['✦', '◇', '◆', '◈'];
const GLITCH_REPAIR_GLYPHS: [char; 4] = ['+', '=', ':', '.'];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct PixelArtPoseKey {
    pub tick: u64,
    pub hold_eyes_closed: bool,
    pub blink_suppression_ticks: u8,
    pub blink_slowdown: u8,
    pub soft_eyes: bool,
    pub work_accent: &'static str,
    pub feed_reaction: bool,
    pub glitch_patch_tier: Option<&'static str>,
    pub glitch_burst_level: Option<&'static str>,
    #[serde(skip)]
    pub glitch_day_key: Option<u64>,
    pub glitch_calm_mode: bool,
    pub glitch_feed_reaction: bool,
}

impl PixelArtPoseKey {
    pub fn from_animation_frame(frame: AnimationFrame) -> Self {
        Self {
            tick: frame.tick,
            hold_eyes_closed: frame.hold_eyes_closed,
            blink_suppression_ticks: frame.blink_suppression_ticks,
            blink_slowdown: frame.blink_slowdown,
            soft_eyes: frame.soft_eyes,
            work_accent: work_accent_label(frame.work_accent),
            feed_reaction: frame.feed_reaction,
            glitch_patch_tier: frame
                .glitch_corruption
                .map(|glitch| glitch.patch_tier.as_str()),
            glitch_burst_level: frame
                .glitch_corruption
                .map(|glitch| glitch.burst_level.as_str()),
            glitch_day_key: frame
                .glitch_corruption
                .map(|glitch| projected_glitch_day_key(glitch.day_seed)),
            glitch_calm_mode: frame
                .glitch_corruption
                .is_some_and(|glitch| glitch.calm_mode),
            glitch_feed_reaction: frame
                .glitch_corruption
                .is_some_and(|glitch| glitch.feed_reaction),
        }
    }
}

fn work_accent_label(accent: WorkAccent) -> &'static str {
    match accent {
        WorkAccent::None => "none",
        WorkAccent::Alert => "alert",
        WorkAccent::Focused => "focused",
        WorkAccent::Dreamy => "dreamy",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelCanonicalAnimationInputs {
    pub tick: u64,
    pub hold_eyes_closed: bool,
    pub blink_suppression_ticks: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum PixelArtRole {
    Body,
    BodyGlow,
    Eye,
    Mouth,
    Accent,
    Pattern,
    Particle,
    Corruption,
    Outline,
    InteriorTexture,
    Appendage,
    FootContact,
    Locket,
    Facet,
    RepairMark,
}

impl PixelArtRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::BodyGlow => "body_glow",
            Self::Eye => "eye",
            Self::Mouth => "mouth",
            Self::Accent => "accent",
            Self::Pattern => "pattern",
            Self::Particle => "particle",
            Self::Corruption => "corruption",
            Self::Outline => "outline",
            Self::InteriorTexture => "interior_texture",
            Self::Appendage => "appendage",
            Self::FootContact => "foot_contact",
            Self::Locket => "locket",
            Self::Facet => "facet",
            Self::RepairMark => "repair_mark",
        }
    }
}

impl From<PaletteRoleName> for PixelArtRole {
    fn from(role: PaletteRoleName) -> Self {
        match role {
            PaletteRoleName::Body => Self::Body,
            PaletteRoleName::BodyGlow => Self::BodyGlow,
            PaletteRoleName::Eye => Self::Eye,
            PaletteRoleName::Mouth => Self::Mouth,
            PaletteRoleName::Accent => Self::Accent,
            PaletteRoleName::Pattern => Self::Pattern,
            PaletteRoleName::Particle => Self::Particle,
            PaletteRoleName::Corruption => Self::Corruption,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct PixelArtCell {
    pub x: u8,
    pub y: u8,
    pub role: PixelArtRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PixelCellBounds {
    pub min_x: u8,
    pub min_y: u8,
    pub max_x: u8,
    pub max_y: u8,
}

impl PixelCellBounds {
    pub const fn width(self) -> u8 {
        self.max_x - self.min_x + 1
    }

    pub const fn height(self) -> u8 {
        self.max_y - self.min_y + 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PixelFootContact {
    pub cells: Vec<(u8, u8)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PixelProtectedRegion {
    pub id: &'static str,
    pub role: &'static str,
    pub bounds: PixelCellBounds,
    pub cell_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PixelCueCoverage {
    pub expected: usize,
    pub present: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PixelReferenceChecksum(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PixelPetArtReference {
    pub species: Species,
    pub stage: Stage,
    pub mood: Mood,
    pub pose: PixelArtPoseKey,
    pub width_cells: u8,
    pub height_cells: u8,
    pub occupied_cells: Vec<PixelArtCell>,
    pub body_bounds: PixelCellBounds,
    pub foot_contact: PixelFootContact,
    pub protected_regions: Vec<PixelProtectedRegion>,
    pub cue_coverage: BTreeMap<&'static str, PixelCueCoverage>,
    pub reference_checksum: PixelReferenceChecksum,
    pub role_counts: BTreeMap<&'static str, usize>,
}

impl PixelPetArtReference {
    pub fn role_count(&self, role: PixelArtRole) -> usize {
        self.role_counts.get(role.as_str()).copied().unwrap_or(0)
    }

    pub fn cells_for_roles<const N: usize>(&self, roles: [PixelArtRole; N]) -> Vec<PixelArtCell> {
        self.occupied_cells
            .iter()
            .copied()
            .filter(|cell| roles.contains(&cell.role))
            .collect()
    }

    pub fn protected_region(&self, id: &str) -> Option<PixelProtectedRegion> {
        self.protected_regions
            .iter()
            .copied()
            .find(|region| region.id == id)
    }

    pub fn cue_coverage(&self, id: &str) -> Option<PixelCueCoverage> {
        self.cue_coverage.get(id).copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelArtReferenceRequest {
    pub seed: String,
    pub species: Species,
    pub stage: Stage,
    pub mood: Mood,
    pub variation_bucket: u16,
    pub pose: PixelArtPoseKey,
    pub animation_frame: AnimationFrame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PixelArtReferenceKey {
    seed: String,
    species: Species,
    stage: Stage,
    mood: Mood,
    variation_bucket: u16,
    pose: PixelArtPoseKey,
}

impl From<&PixelArtReferenceRequest> for PixelArtReferenceKey {
    fn from(request: &PixelArtReferenceRequest) -> Self {
        Self {
            seed: request.seed.clone(),
            species: request.species,
            stage: request.stage,
            mood: request.mood,
            variation_bucket: request.variation_bucket,
            pose: request.pose,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct PixelArtReferenceProvider {
    cached: Option<(PixelArtReferenceKey, PixelPetArtReference)>,
    render_count_for_test: usize,
}

impl PixelArtReferenceProvider {
    pub fn reference_for(&mut self, request: &PixelArtReferenceRequest) -> PixelPetArtReference {
        let key = PixelArtReferenceKey::from(request);
        if let Some((cached_key, cached_reference)) = &self.cached {
            if *cached_key == key {
                return cached_reference.clone();
            }
        }

        let reference = render_reference(request);
        self.render_count_for_test += 1;
        self.cached = Some((key, reference.clone()));
        reference
    }

    pub fn render_count_for_test(&self) -> usize {
        self.render_count_for_test
    }
}

fn render_reference(request: &PixelArtReferenceRequest) -> PixelPetArtReference {
    let generated = generate_pet(&request.seed).with_species(request.species);
    let rendered = crate::pet::render::render_pet(
        &generated,
        request.stage,
        request.mood,
        request.animation_frame,
    );

    let height_cells = rendered.lines.len() as u8;
    let width_cells = rendered
        .lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0) as u8;

    let mut occupied_cells = Vec::new();
    for (y, line) in rendered.lines.iter().enumerate() {
        for (x, glyph) in line.chars().enumerate() {
            if glyph.is_whitespace() {
                continue;
            }
            occupied_cells.push(PixelArtCell {
                x: x as u8,
                y: y as u8,
                role: role_at(&rendered.spans, y, x).into(),
            });
        }
    }
    occupied_cells.sort_by_key(|cell| (cell.y, cell.x, cell.role.as_str()));

    let structural_cells = occupied_cells
        .iter()
        .copied()
        .filter(|cell| cell.role != PixelArtRole::Particle)
        .collect::<Vec<_>>();
    let body_bounds = bounds_for(&structural_cells).unwrap_or(PixelCellBounds {
        min_x: 0,
        min_y: 0,
        max_x: 0,
        max_y: 0,
    });
    let footprint = structural_cells
        .iter()
        .map(|cell| (cell.x, cell.y))
        .collect::<BTreeSet<_>>();
    let foot_contact = PixelFootContact {
        cells: foot_contact_cells(&structural_cells),
    };
    let base_occupied_cells = occupied_cells.clone();
    let mut occupied_cells = promote_reference_cells(
        request,
        &rendered.lines,
        occupied_cells,
        &footprint,
        &foot_contact,
    );
    occupied_cells.sort_by_key(|cell| (cell.y, cell.x, cell.role.as_str()));

    let role_counts = role_counts_for(&occupied_cells);
    let cue_coverage = cue_coverage_for(
        request,
        &rendered.lines,
        &base_occupied_cells,
        &occupied_cells,
    );
    let protected_regions = protected_regions_for(&occupied_cells);

    let reference_checksum =
        PixelReferenceChecksum(reference_checksum(request, &occupied_cells, body_bounds));

    PixelPetArtReference {
        species: request.species,
        stage: request.stage,
        mood: request.mood,
        pose: request.pose,
        width_cells,
        height_cells,
        occupied_cells,
        body_bounds,
        foot_contact,
        protected_regions,
        cue_coverage,
        reference_checksum,
        role_counts,
    }
}

fn role_at(spans: &[crate::pet::render::StyledSegment], row: usize, col: usize) -> PaletteRoleName {
    spans
        .iter()
        .find(|span| span.line == row && col >= span.start && col < span.end)
        .map(|span| span.role)
        .unwrap_or(PaletteRoleName::Body)
}

fn bounds_for(cells: &[PixelArtCell]) -> Option<PixelCellBounds> {
    let mut iter = cells.iter();
    let first = iter.next()?;
    let mut bounds = PixelCellBounds {
        min_x: first.x,
        min_y: first.y,
        max_x: first.x,
        max_y: first.y,
    };
    for cell in iter {
        bounds.min_x = bounds.min_x.min(cell.x);
        bounds.min_y = bounds.min_y.min(cell.y);
        bounds.max_x = bounds.max_x.max(cell.x);
        bounds.max_y = bounds.max_y.max(cell.y);
    }
    Some(bounds)
}

fn foot_contact_cells(cells: &[PixelArtCell]) -> Vec<(u8, u8)> {
    let Some(max_y) = cells.iter().map(|cell| cell.y).max() else {
        return Vec::new();
    };
    let mut contact = cells
        .iter()
        .filter(|cell| cell.y == max_y)
        .map(|cell| (cell.x, cell.y))
        .collect::<Vec<_>>();
    contact.sort_unstable();
    contact.dedup();
    contact
}

fn promote_reference_cells(
    request: &PixelArtReferenceRequest,
    lines: &[String],
    cells: Vec<PixelArtCell>,
    footprint: &BTreeSet<(u8, u8)>,
    foot_contact: &PixelFootContact,
) -> Vec<PixelArtCell> {
    cells
        .into_iter()
        .map(|mut cell| {
            if cell.role == PixelArtRole::Particle {
                return cell;
            }
            if matches!(cell.role, PixelArtRole::Eye | PixelArtRole::Mouth) {
                return cell;
            }

            let glyph = glyph_at(lines, cell.x, cell.y);
            cell.role = promoted_role_for(request, cell, glyph, footprint, foot_contact);
            cell
        })
        .collect()
}

fn promoted_role_for(
    request: &PixelArtReferenceRequest,
    cell: PixelArtCell,
    glyph: char,
    footprint: &BTreeSet<(u8, u8)>,
    foot_contact: &PixelFootContact,
) -> PixelArtRole {
    if request.species == Species::Fuzz && LOCKET_GLYPHS.contains(&glyph) {
        return PixelArtRole::Locket;
    }
    if request.species == Species::Crystal && FACET_GLYPHS.contains(&glyph) {
        return PixelArtRole::Facet;
    }
    if request.species == Species::Glitch
        && matches!(request.stage, Stage::S4 | Stage::S5 | Stage::S6)
        && matches!(cell.role, PixelArtRole::Accent | PixelArtRole::Pattern)
        && GLITCH_REPAIR_GLYPHS.contains(&glyph)
    {
        return PixelArtRole::RepairMark;
    }
    if matches!(
        cell.role,
        PixelArtRole::Corruption | PixelArtRole::Pattern | PixelArtRole::Accent
    ) {
        return cell.role;
    }
    if foot_contact.cells.contains(&(cell.x, cell.y)) {
        return PixelArtRole::FootContact;
    }
    if is_appendage_cell(&cell, footprint) {
        return PixelArtRole::Appendage;
    }
    if matches!(cell.role, PixelArtRole::Body | PixelArtRole::BodyGlow) {
        if is_outline_cell(cell.x, cell.y, footprint) {
            return PixelArtRole::Outline;
        }
        return PixelArtRole::InteriorTexture;
    }
    cell.role
}

fn role_counts_for(cells: &[PixelArtCell]) -> BTreeMap<&'static str, usize> {
    let mut role_counts = BTreeMap::new();
    for cell in cells {
        *role_counts.entry(cell.role.as_str()).or_insert(0) += 1;
    }
    role_counts
}

fn cue_coverage_for(
    request: &PixelArtReferenceRequest,
    lines: &[String],
    base_cells: &[PixelArtCell],
    promoted_cells: &[PixelArtCell],
) -> BTreeMap<&'static str, PixelCueCoverage> {
    let mut coverage = BTreeMap::new();
    coverage.insert(
        "locket",
        coverage_for_glyph_role(
            request,
            lines,
            base_cells,
            promoted_cells,
            Species::Fuzz,
            &LOCKET_GLYPHS,
            PixelArtRole::Locket,
        ),
    );
    coverage.insert(
        "facet",
        coverage_for_glyph_role(
            request,
            lines,
            base_cells,
            promoted_cells,
            Species::Crystal,
            &FACET_GLYPHS,
            PixelArtRole::Facet,
        ),
    );
    coverage.insert(
        "repair_mark",
        glitch_repair_coverage(request, lines, base_cells, promoted_cells),
    );
    coverage
}

fn coverage_for_glyph_role(
    request: &PixelArtReferenceRequest,
    lines: &[String],
    base_cells: &[PixelArtCell],
    promoted_cells: &[PixelArtCell],
    species: Species,
    glyphs: &[char],
    role: PixelArtRole,
) -> PixelCueCoverage {
    if request.species != species {
        return PixelCueCoverage { expected: 0, present: 0 };
    }
    let expected = base_cells
        .iter()
        .filter(|cell| glyphs.contains(&glyph_at(lines, cell.x, cell.y)))
        .count();
    let present = promoted_cells
        .iter()
        .filter(|cell| cell.role == role)
        .count();
    PixelCueCoverage { expected, present }
}

fn glitch_repair_coverage(
    request: &PixelArtReferenceRequest,
    lines: &[String],
    base_cells: &[PixelArtCell],
    promoted_cells: &[PixelArtCell],
) -> PixelCueCoverage {
    if request.species != Species::Glitch
        || !matches!(request.stage, Stage::S4 | Stage::S5 | Stage::S6)
    {
        return PixelCueCoverage { expected: 0, present: 0 };
    }
    let expected = base_cells
        .iter()
        .filter(|cell| {
            matches!(cell.role, PixelArtRole::Accent | PixelArtRole::Pattern)
                && GLITCH_REPAIR_GLYPHS.contains(&glyph_at(lines, cell.x, cell.y))
        })
        .count();
    let present = promoted_cells
        .iter()
        .filter(|cell| cell.role == PixelArtRole::RepairMark)
        .count();
    PixelCueCoverage { expected, present }
}

fn protected_regions_for(cells: &[PixelArtCell]) -> Vec<PixelProtectedRegion> {
    let groups: [(&str, &str, &[PixelArtRole]); 4] = [
        ("face", "face", &[PixelArtRole::Eye, PixelArtRole::Mouth]),
        ("signature-locket", "signature", &[PixelArtRole::Locket]),
        ("signature-facet", "signature", &[PixelArtRole::Facet]),
        (
            "signature-repair-mark",
            "signature",
            &[PixelArtRole::RepairMark],
        ),
    ];
    groups
        .into_iter()
        .filter_map(|(id, role, roles)| protected_region_for_roles(id, role, cells, roles))
        .collect()
}

fn protected_region_for_roles(
    id: &'static str,
    role: &'static str,
    cells: &[PixelArtCell],
    roles: &[PixelArtRole],
) -> Option<PixelProtectedRegion> {
    let matching = cells
        .iter()
        .copied()
        .filter(|cell| roles.contains(&cell.role))
        .collect::<Vec<_>>();
    let bounds = bounds_for(&matching)?;
    Some(PixelProtectedRegion {
        id,
        role,
        bounds,
        cell_count: matching.len(),
    })
}

fn glyph_at(lines: &[String], x: u8, y: u8) -> char {
    lines[usize::from(y)]
        .chars()
        .nth(usize::from(x))
        .unwrap_or(' ')
}

fn is_outline_cell(x: u8, y: u8, footprint: &BTreeSet<(u8, u8)>) -> bool {
    let x = i16::from(x);
    let y = i16::from(y);
    [(-1, 0), (1, 0), (0, -1), (0, 1)]
        .into_iter()
        .any(|(dx, dy)| !footprint.contains(&((x + dx) as u8, (y + dy) as u8)))
}

fn is_appendage_cell(cell: &PixelArtCell, footprint: &BTreeSet<(u8, u8)>) -> bool {
    if !matches!(cell.role, PixelArtRole::Body | PixelArtRole::BodyGlow) {
        return false;
    }
    let x = i16::from(cell.x);
    let y = i16::from(cell.y);
    let horizontal_neighbors = [(-1, 0), (1, 0)]
        .into_iter()
        .filter(|(dx, dy)| footprint.contains(&((x + dx) as u8, (y + dy) as u8)))
        .count();
    let vertical_neighbors = [(0, -1), (0, 1)]
        .into_iter()
        .filter(|(dx, dy)| footprint.contains(&((x + dx) as u8, (y + dy) as u8)))
        .count();
    horizontal_neighbors + vertical_neighbors <= 1
}

fn reference_checksum(
    request: &PixelArtReferenceRequest,
    occupied_cells: &[PixelArtCell],
    body_bounds: PixelCellBounds,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash = hash_bytes(hash, request.species.as_str().as_bytes());
    hash = hash_bytes(hash, request.stage.as_str().as_bytes());
    hash = hash_bytes(hash, request.mood.as_str().as_bytes());
    hash = hash_u64(hash, request.pose.tick);
    hash = hash_u8(hash, request.pose.hold_eyes_closed as u8);
    hash = hash_u8(hash, request.pose.blink_suppression_ticks);
    hash = hash_u8(hash, request.pose.blink_slowdown);
    hash = hash_u8(hash, request.pose.soft_eyes as u8);
    hash = hash_bytes(hash, request.pose.work_accent.as_bytes());
    hash = hash_u8(hash, request.pose.feed_reaction as u8);
    hash = hash_optional_str(hash, request.pose.glitch_patch_tier);
    hash = hash_optional_str(hash, request.pose.glitch_burst_level);
    hash = hash_optional_u64(hash, request.pose.glitch_day_key);
    hash = hash_u8(hash, request.pose.glitch_calm_mode as u8);
    hash = hash_u8(hash, request.pose.glitch_feed_reaction as u8);
    for cell in occupied_cells {
        hash = hash_u8(hash, cell.x);
        hash = hash_u8(hash, cell.y);
        hash = hash_bytes(hash, cell.role.as_str().as_bytes());
    }
    hash = hash_u8(hash, body_bounds.min_x);
    hash = hash_u8(hash, body_bounds.min_y);
    hash = hash_u8(hash, body_bounds.max_x);
    hash = hash_u8(hash, body_bounds.max_y);
    hash
}

fn hash_optional_str(hash: u64, value: Option<&'static str>) -> u64 {
    match value {
        Some(value) => hash_bytes(hash_u8(hash, 1), value.as_bytes()),
        None => hash_u8(hash, 0),
    }
}

fn projected_glitch_day_key(day_seed: u64) -> u64 {
    hash_u64(
        hash_bytes(0xcbf2_9ce4_8422_2325_u64, b"glitch-day"),
        day_seed,
    )
}

fn hash_optional_u64(hash: u64, value: Option<u64>) -> u64 {
    match value {
        Some(value) => hash_u64(hash_u8(hash, 1), value),
        None => hash_u8(hash, 0),
    }
}

fn hash_u8(hash: u64, value: u8) -> u64 {
    hash_bytes(hash, &[value])
}

fn hash_u64(mut hash: u64, value: u64) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn priority_request(species: Species) -> PixelArtReferenceRequest {
        let animation_frame = AnimationFrame::default();
        PixelArtReferenceRequest {
            seed: "priority-test".to_string(),
            species,
            stage: Stage::S3,
            mood: Mood::Content,
            variation_bucket: 0,
            pose: PixelArtPoseKey::from_animation_frame(animation_frame),
            animation_frame,
        }
    }

    #[test]
    fn foot_contact_promotion_preserves_existing_species_and_accent_roles() {
        let request = priority_request(Species::Mech);
        let footprint = BTreeSet::from([(2, 4)]);
        let foot_contact = PixelFootContact { cells: vec![(2, 4)] };

        for role in [
            PixelArtRole::Corruption,
            PixelArtRole::Pattern,
            PixelArtRole::Accent,
        ] {
            let promoted = promoted_role_for(
                &request,
                PixelArtCell { x: 2, y: 4, role },
                '#',
                &footprint,
                &foot_contact,
            );

            assert_eq!(promoted, role, "{role:?} must outrank foot contact");
        }
    }

    #[test]
    fn signature_promotion_still_outranks_existing_accent_foot_contact() {
        let request = priority_request(Species::Fuzz);
        let footprint = BTreeSet::from([(2, 4)]);
        let foot_contact = PixelFootContact { cells: vec![(2, 4)] };

        let promoted = promoted_role_for(
            &request,
            PixelArtCell { x: 2, y: 4, role: PixelArtRole::Accent },
            '◆',
            &footprint,
            &foot_contact,
        );

        assert_eq!(promoted, PixelArtRole::Locket);
    }
}
