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
    Backdrop,
    Body,
    Face,
    Glow,
    Prop,
    Room,
    Overlay,
}

impl SmoothLayerRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Backdrop => "backdrop",
            Self::Body => "body",
            Self::Face => "face",
            Self::Glow => "glow",
            Self::Prop => "prop",
            Self::Room => "room",
            Self::Overlay => "overlay",
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmoothCompanionLayer {
    pub id: SmoothLayerId,
    pub role: SmoothLayerRole,
    pub z: i16,
    pub items: Vec<SmoothLayerItem>,
    pub privacy: SmoothCompanionPrivacyClaims,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayeredPetScene {
    pub layers: Vec<SmoothCompanionLayer>,
}

pub fn flatten_classic_cells(scene: &LayeredPetScene) -> SceneDrawList {
    let mut ordered_layers: Vec<(usize, &SmoothCompanionLayer)> =
        scene.layers.iter().enumerate().collect();
    ordered_layers.sort_by_key(|(index, layer)| (layer.z, *index));

    let mut cells = Vec::new();
    for (_, layer) in ordered_layers {
        for item in &layer.items {
            if let SmoothLayerItem::LocalCell(cell) = item {
                cells.push(DrawCell {
                    row: cell.row,
                    col: cell.col,
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
                    SmoothLayerRole::Overlay,
                    2,
                    vec![
                        local_item(cell(0, 2, "A", Some(rgb(1, 2, 3)), None, false)),
                        local_item(cell(0, 3, "B", Some(rgb(4, 5, 6)), None, true)),
                    ],
                ),
                layer(
                    "first-low",
                    SmoothLayerRole::Body,
                    0,
                    vec![local_item(cell(1, 4, "C", Some(rgb(7, 8, 9)), None, false))],
                ),
                layer(
                    "middle-high",
                    SmoothLayerRole::Face,
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

        let flattened = flatten_classic_cells(&scene);

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
    fn smooth_layer_role_as_str_is_kebab_case() {
        assert_eq!(SmoothLayerRole::Backdrop.as_str(), "backdrop");
        assert_eq!(SmoothLayerRole::Body.as_str(), "body");
        assert_eq!(SmoothLayerRole::Face.as_str(), "face");
        assert_eq!(SmoothLayerRole::Glow.as_str(), "glow");
        assert_eq!(SmoothLayerRole::Prop.as_str(), "prop");
        assert_eq!(SmoothLayerRole::Room.as_str(), "room");
        assert_eq!(SmoothLayerRole::Overlay.as_str(), "overlay");
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
