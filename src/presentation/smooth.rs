use crate::pet::palette::Rgb;
use crate::presentation::{DrawCell, SceneDrawList};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothBounds {
    pub min: SmoothPoint,
    pub max: SmoothPoint,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothTransform {
    pub translation: SmoothPoint,
    pub scale: SmoothPoint,
    pub rotation_degrees: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SmoothClip {
    None,
    Rect(SmoothBounds),
    Circle { center: SmoothPoint, radius: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothBlendMode {
    Normal,
    Multiply,
    Screen,
    Add,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmoothLayerId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothLayerRole {
    DepthRings,
    BiomeWash,
    RoomGlyphs,
    Ambient,
    Motes,
    ActivityGlyphs,
    PropsBehind,
    TankLifeBehind,
    ChestBubble,
    ContactShadow,
    PetBody,
    PerformanceCue,
    PropsForeground,
    TankLifeForeground,
    StatusHalo,
    TroubleIndicator,
    MoodAura,
    DimOverlay,
}

impl SmoothLayerRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DepthRings => "depth-rings",
            Self::BiomeWash => "biome-wash",
            Self::RoomGlyphs => "room-glyphs",
            Self::Ambient => "ambient",
            Self::Motes => "motes",
            Self::ActivityGlyphs => "activity-glyphs",
            Self::PropsBehind => "props-behind",
            Self::TankLifeBehind => "tank-life-behind",
            Self::ChestBubble => "chest-bubble",
            Self::ContactShadow => "contact-shadow",
            Self::PetBody => "pet-body",
            Self::PerformanceCue => "performance-cue",
            Self::PropsForeground => "props-foreground",
            Self::TankLifeForeground => "tank-life-foreground",
            Self::StatusHalo => "status-halo",
            Self::TroubleIndicator => "trouble-indicator",
            Self::MoodAura => "mood-aura",
            Self::DimOverlay => "dim-overlay",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmoothLocalCell {
    pub row: u16,
    pub col: u16,
    pub glyph: Option<String>,
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub bold: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmoothShapeRef {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmoothRasterRef {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmoothLayerItem {
    LocalCell(SmoothLocalCell),
    Shape(SmoothShapeRef),
    Raster(SmoothRasterRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmoothCompanionPrivacyClaims {
    pub source_names_visible: bool,
    pub exact_token_strings_visible: bool,
    pub project_names_visible: bool,
    pub file_paths_visible: bool,
    pub prompt_text_visible: bool,
    pub response_text_visible: bool,
    pub raw_diagnostics_visible: bool,
    pub unprojected_pet_seed_visible: bool,
}

impl SmoothCompanionPrivacyClaims {
    pub fn external_companion() -> Self {
        Self {
            source_names_visible: false,
            exact_token_strings_visible: false,
            project_names_visible: false,
            file_paths_visible: false,
            prompt_text_visible: false,
            response_text_visible: false,
            raw_diagnostics_visible: false,
            unprojected_pet_seed_visible: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmoothCompanionLayer {
    pub id: SmoothLayerId,
    pub role: SmoothLayerRole,
    pub z: i16,
    pub local_bounds: SmoothBounds,
    pub anchor: SmoothPoint,
    pub transform_origin: SmoothPoint,
    pub transform: SmoothTransform,
    pub opacity: f32,
    pub clip: SmoothClip,
    pub blend: SmoothBlendMode,
    pub items: Vec<SmoothLayerItem>,
    pub privacy: SmoothCompanionPrivacyClaims,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayeredPetScene {
    pub layers: Vec<SmoothCompanionLayer>,
}

impl LayeredPetScene {
    pub fn flatten_classic_cells(&self) -> SceneDrawList {
        flatten_layers_to_draw_list(&self.layers)
    }

    pub fn classic_flatten_checksum(&self) -> u64 {
        classic_flatten_checksum(&self.flatten_classic_cells().cells)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmoothCompanionScenePlan {
    pub layers: Vec<SmoothCompanionLayer>,
    pub privacy: SmoothCompanionPrivacyClaims,
}

impl SmoothCompanionScenePlan {
    pub fn flatten_classic_cells(&self) -> SceneDrawList {
        flatten_layers_to_draw_list(&self.layers)
    }

    pub fn classic_flatten_checksum(&self) -> u64 {
        classic_flatten_checksum(&self.flatten_classic_cells().cells)
    }
}

pub fn flatten_classic_cells(scene: &LayeredPetScene) -> SceneDrawList {
    scene.flatten_classic_cells()
}

fn flatten_layers_to_draw_list(layers: &[SmoothCompanionLayer]) -> SceneDrawList {
    let mut ordered_layers: Vec<(usize, &SmoothCompanionLayer)> =
        layers.iter().enumerate().collect();
    ordered_layers.sort_by_key(|(index, layer)| (layer.z, *index));

    let mut cells = Vec::new();
    for (_, layer) in ordered_layers {
        for item in &layer.items {
            if let SmoothLayerItem::LocalCell(cell) = item {
                cells.push(DrawCell {
                    row: classic_cell_axis(
                        layer.anchor.y + layer.transform.translation.y + f32::from(cell.row),
                    ),
                    col: classic_cell_axis(
                        layer.anchor.x + layer.transform.translation.x + f32::from(cell.col),
                    ),
                    glyph: cell.glyph.clone(),
                    fg: cell.fg,
                    bg: cell.bg,
                    bold: cell.bold,
                });
            }
        }
    }

    SceneDrawList { cells }
}

fn classic_cell_axis(value: f32) -> u16 {
    if !value.is_finite() {
        return 0;
    }

    value.round().clamp(0.0, f32::from(u16::MAX)) as u16
}

pub fn classic_flatten_checksum(cells: &[DrawCell]) -> u64 {
    let mut hash = FNV_OFFSET;
    hash = hash_bytes(hash, b"classic-flatten");
    for cell in cells {
        hash = hash_u16(hash, cell.row);
        hash = hash_u16(hash, cell.col);
        hash = hash_optional_str(hash, cell.glyph.as_deref());
        hash = hash_optional_rgb(hash, cell.fg);
        hash = hash_optional_rgb(hash, cell.bg);
        hash = hash_u8(hash, cell.bold as u8);
    }
    hash
}

pub fn smooth_pet_bob(elapsed_ms: u64) -> f32 {
    const AMPLITUDE: f32 = 0.33;
    const PERIOD_MS: f32 = 2_000.0;
    let phase = (elapsed_ms as f32 / PERIOD_MS) * std::f32::consts::TAU;
    phase.sin() * AMPLITUDE
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn hash_u8(hash: u64, value: u8) -> u64 {
    hash_bytes(hash, &[value])
}

fn hash_u16(hash: u64, value: u16) -> u64 {
    hash_bytes(hash, &value.to_le_bytes())
}

fn hash_u64(hash: u64, value: u64) -> u64 {
    hash_bytes(hash, &value.to_le_bytes())
}

fn hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn hash_optional_str(hash: u64, value: Option<&str>) -> u64 {
    match value {
        Some(value) => hash_bytes(hash_u8(hash, 1), value.as_bytes()),
        None => hash_u8(hash, 0),
    }
}

fn hash_optional_rgb(hash: u64, value: Option<Rgb>) -> u64 {
    match value {
        Some(value) => hash_u64(
            hash_u8(hash, 1),
            u64::from(value.r) << 16 | u64::from(value.g) << 8 | u64::from(value.b),
        ),
        None => hash_u8(hash, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::palette::Rgb;
    use crate::presentation::{DrawCell, SceneDrawList};

    fn rgb(r: u8, g: u8, b: u8) -> Rgb {
        Rgb::new(r, g, b)
    }

    fn cell(
        row: u16,
        col: u16,
        glyph: &str,
        fg: Option<Rgb>,
        bg: Option<Rgb>,
        bold: bool,
    ) -> SmoothLocalCell {
        SmoothLocalCell {
            row,
            col,
            glyph: Some(glyph.to_string()),
            fg,
            bg,
            bold,
        }
    }

    fn local_item(cell: SmoothLocalCell) -> SmoothLayerItem {
        SmoothLayerItem::LocalCell(cell)
    }

    fn layer(
        id: &str,
        role: SmoothLayerRole,
        z: i16,
        items: Vec<SmoothLayerItem>,
    ) -> SmoothCompanionLayer {
        SmoothCompanionLayer {
            id: SmoothLayerId(id.to_string()),
            role,
            z,
            local_bounds: SmoothBounds {
                min: SmoothPoint { x: 0.0, y: 0.0 },
                max: SmoothPoint { x: 8.0, y: 8.0 },
            },
            anchor: SmoothPoint { x: 0.0, y: 0.0 },
            transform_origin: SmoothPoint { x: 0.5, y: 0.5 },
            transform: SmoothTransform {
                translation: SmoothPoint { x: 0.0, y: 0.0 },
                scale: SmoothPoint { x: 1.0, y: 1.0 },
                rotation_degrees: 0.0,
            },
            opacity: 1.0,
            clip: SmoothClip::None,
            blend: SmoothBlendMode::Normal,
            items,
            privacy: SmoothCompanionPrivacyClaims::external_companion(),
        }
    }

    #[test]
    fn flatten_classic_cells_sorts_by_z_then_layer_index_and_keeps_item_order() {
        let scene = LayeredPetScene {
            layers: vec![
                layer(
                    "later-high",
                    SmoothLayerRole::DimOverlay,
                    2,
                    vec![
                        local_item(cell(0, 2, "A", Some(rgb(1, 2, 3)), None, false)),
                        local_item(cell(0, 3, "B", Some(rgb(4, 5, 6)), None, true)),
                    ],
                ),
                layer(
                    "first-low",
                    SmoothLayerRole::PetBody,
                    0,
                    vec![local_item(cell(1, 4, "C", Some(rgb(7, 8, 9)), None, false))],
                ),
                layer(
                    "middle-high",
                    SmoothLayerRole::PerformanceCue,
                    2,
                    vec![local_item(cell(
                        0,
                        5,
                        "D",
                        Some(rgb(10, 11, 12)),
                        None,
                        false,
                    ))],
                ),
            ],
        };

        let flattened = scene.flatten_classic_cells();

        assert_eq!(
            flattened,
            SceneDrawList {
                cells: vec![
                    DrawCell {
                        row: 1,
                        col: 4,
                        glyph: Some("C".to_string()),
                        fg: Some(rgb(7, 8, 9)),
                        bg: None,
                        bold: false,
                    },
                    DrawCell {
                        row: 0,
                        col: 2,
                        glyph: Some("A".to_string()),
                        fg: Some(rgb(1, 2, 3)),
                        bg: None,
                        bold: false,
                    },
                    DrawCell {
                        row: 0,
                        col: 3,
                        glyph: Some("B".to_string()),
                        fg: Some(rgb(4, 5, 6)),
                        bg: None,
                        bold: true,
                    },
                    DrawCell {
                        row: 0,
                        col: 5,
                        glyph: Some("D".to_string()),
                        fg: Some(rgb(10, 11, 12)),
                        bg: None,
                        bold: false,
                    },
                ],
            }
        );
    }

    #[test]
    fn flatten_classic_cells_projects_local_cells_through_anchor_and_translation() {
        let layer = SmoothCompanionLayer {
            id: SmoothLayerId("pet-body".to_string()),
            role: SmoothLayerRole::PetBody,
            z: 0,
            local_bounds: SmoothBounds {
                min: SmoothPoint { x: 0.0, y: 0.0 },
                max: SmoothPoint { x: 8.0, y: 8.0 },
            },
            anchor: SmoothPoint { x: 10.0, y: 20.0 },
            transform_origin: SmoothPoint { x: 0.0, y: 0.0 },
            transform: SmoothTransform {
                translation: SmoothPoint { x: 3.0, y: 2.0 },
                scale: SmoothPoint { x: 1.0, y: 1.0 },
                rotation_degrees: 0.0,
            },
            opacity: 1.0,
            clip: SmoothClip::None,
            blend: SmoothBlendMode::Normal,
            items: vec![
                local_item(cell(1, 4, "X", Some(rgb(1, 2, 3)), None, false)),
                local_item(cell(0, 0, "Y", Some(rgb(4, 5, 6)), None, true)),
            ],
            privacy: SmoothCompanionPrivacyClaims::external_companion(),
        };
        let scene = LayeredPetScene { layers: vec![layer.clone()] };
        let plan = SmoothCompanionScenePlan {
            layers: vec![layer],
            privacy: SmoothCompanionPrivacyClaims::external_companion(),
        };
        let expected = SceneDrawList {
            cells: vec![
                DrawCell {
                    row: 23,
                    col: 17,
                    glyph: Some("X".to_string()),
                    fg: Some(rgb(1, 2, 3)),
                    bg: None,
                    bold: false,
                },
                DrawCell {
                    row: 22,
                    col: 13,
                    glyph: Some("Y".to_string()),
                    fg: Some(rgb(4, 5, 6)),
                    bg: None,
                    bold: true,
                },
            ],
        };

        assert_eq!(scene.flatten_classic_cells(), expected);
        assert_eq!(plan.flatten_classic_cells(), expected);
    }

    #[test]
    fn smooth_layer_role_as_str_matches_slice_one_contract() {
        let roles = [
            (SmoothLayerRole::DepthRings, "depth-rings"),
            (SmoothLayerRole::BiomeWash, "biome-wash"),
            (SmoothLayerRole::RoomGlyphs, "room-glyphs"),
            (SmoothLayerRole::Ambient, "ambient"),
            (SmoothLayerRole::Motes, "motes"),
            (SmoothLayerRole::ActivityGlyphs, "activity-glyphs"),
            (SmoothLayerRole::PropsBehind, "props-behind"),
            (SmoothLayerRole::TankLifeBehind, "tank-life-behind"),
            (SmoothLayerRole::ChestBubble, "chest-bubble"),
            (SmoothLayerRole::ContactShadow, "contact-shadow"),
            (SmoothLayerRole::PetBody, "pet-body"),
            (SmoothLayerRole::PerformanceCue, "performance-cue"),
            (SmoothLayerRole::PropsForeground, "props-foreground"),
            (SmoothLayerRole::TankLifeForeground, "tank-life-foreground"),
            (SmoothLayerRole::StatusHalo, "status-halo"),
            (SmoothLayerRole::TroubleIndicator, "trouble-indicator"),
            (SmoothLayerRole::MoodAura, "mood-aura"),
            (SmoothLayerRole::DimOverlay, "dim-overlay"),
        ];

        for (role, expected) in roles {
            assert_eq!(role.as_str(), expected);
        }
    }

    #[test]
    fn smooth_companion_layer_can_represent_fractional_transform_contract() {
        let layer = SmoothCompanionLayer {
            id: SmoothLayerId("pet-body".to_string()),
            role: SmoothLayerRole::PetBody,
            z: 9,
            local_bounds: SmoothBounds {
                min: SmoothPoint { x: -1.5, y: -0.25 },
                max: SmoothPoint { x: 4.5, y: 5.75 },
            },
            anchor: SmoothPoint { x: 0.5, y: 1.0 },
            transform_origin: SmoothPoint { x: 0.5, y: 0.8 },
            transform: SmoothTransform {
                translation: SmoothPoint { x: 0.0, y: 0.33 },
                scale: SmoothPoint { x: 1.0, y: 0.96 },
                rotation_degrees: -2.5,
            },
            opacity: 0.85,
            clip: SmoothClip::Rect(SmoothBounds {
                min: SmoothPoint { x: -2.0, y: -1.0 },
                max: SmoothPoint { x: 6.0, y: 7.0 },
            }),
            blend: SmoothBlendMode::Multiply,
            items: vec![local_item(cell(1, 2, "@", Some(rgb(1, 2, 3)), None, true))],
            privacy: SmoothCompanionPrivacyClaims::external_companion(),
        };

        assert_eq!(layer.role, SmoothLayerRole::PetBody);
        assert_eq!(layer.local_bounds.min.x, -1.5);
        assert_eq!(layer.anchor.y, 1.0);
        assert_eq!(layer.transform_origin.y, 0.8);
        assert_eq!(layer.transform.translation.y, 0.33);
        assert_eq!(layer.transform.scale.y, 0.96);
        assert_eq!(layer.transform.rotation_degrees, -2.5);
        assert_eq!(layer.opacity, 0.85);
        assert_eq!(
            layer.clip,
            SmoothClip::Rect(SmoothBounds {
                min: SmoothPoint { x: -2.0, y: -1.0 },
                max: SmoothPoint { x: 6.0, y: 7.0 },
            })
        );
        assert_eq!(layer.blend, SmoothBlendMode::Multiply);
    }

    #[test]
    fn smooth_scene_plan_and_layered_scene_offer_method_flatten_helpers() {
        let layered_scene = LayeredPetScene {
            layers: vec![layer(
                "pet-body",
                SmoothLayerRole::PetBody,
                0,
                vec![local_item(cell(2, 3, "P", Some(rgb(9, 8, 7)), None, false))],
            )],
        };
        let plan = SmoothCompanionScenePlan {
            layers: layered_scene.layers.clone(),
            privacy: SmoothCompanionPrivacyClaims::external_companion(),
        };

        let expected = SceneDrawList {
            cells: vec![DrawCell {
                row: 2,
                col: 3,
                glyph: Some("P".to_string()),
                fg: Some(rgb(9, 8, 7)),
                bg: None,
                bold: false,
            }],
        };

        assert_eq!(layered_scene.flatten_classic_cells(), expected);
        assert_eq!(plan.flatten_classic_cells(), expected);
        assert_eq!(
            layered_scene.classic_flatten_checksum(),
            classic_flatten_checksum(&expected.cells)
        );
        assert_eq!(
            plan.classic_flatten_checksum(),
            classic_flatten_checksum(&expected.cells)
        );
    }

    #[test]
    fn smooth_pet_bob_is_deterministic_fractional_and_bounded() {
        let samples = [smooth_pet_bob(0), smooth_pet_bob(250), smooth_pet_bob(500)];
        let repeated = [smooth_pet_bob(0), smooth_pet_bob(250), smooth_pet_bob(500)];

        assert_eq!(samples, repeated);
        assert!(samples.iter().all(|value| value.abs() < 0.5));
        assert!(samples.iter().any(|value| value.abs() > f32::EPSILON));
        assert!(samples
            .iter()
            .any(|value| (*value - value.round()).abs() > f32::EPSILON));
    }

    #[test]
    fn external_companion_claims_redact_private_fields() {
        let claims = SmoothCompanionPrivacyClaims::external_companion();

        assert!(!claims.source_names_visible);
        assert!(!claims.exact_token_strings_visible);
        assert!(!claims.project_names_visible);
        assert!(!claims.file_paths_visible);
        assert!(!claims.prompt_text_visible);
        assert!(!claims.response_text_visible);
        assert!(!claims.raw_diagnostics_visible);
        assert!(!claims.unprojected_pet_seed_visible);
    }

    #[test]
    fn classic_flatten_checksum_is_stable_and_sensitive() {
        let cells = vec![
            DrawCell {
                row: 0,
                col: 0,
                glyph: Some("A".to_string()),
                fg: Some(rgb(1, 2, 3)),
                bg: Some(rgb(4, 5, 6)),
                bold: false,
            },
            DrawCell {
                row: 1,
                col: 1,
                glyph: None,
                fg: None,
                bg: Some(rgb(7, 8, 9)),
                bold: true,
            },
        ];

        let checksum_a = classic_flatten_checksum(&cells);
        let checksum_b = classic_flatten_checksum(&cells);
        assert_eq!(checksum_a, checksum_b);

        let mut tweaked = cells.clone();
        tweaked[0].bg = Some(rgb(4, 5, 7));
        assert_ne!(checksum_a, classic_flatten_checksum(&tweaked));
    }
}
