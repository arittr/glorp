use serde::{Deserialize, Serialize};

pub const FIXTURE_ID: &str = "renderer-decision-companion-v1";
pub const FIXTURE_SCHEMA_VERSION: u16 = 1;
pub const PET_GLYPH_COUNT: usize = 180;
pub const STATIC_GLYPH_COUNT: usize = 80;
pub const SHAPE_COUNT: usize = 40;
pub const DYNAMIC_PRIMITIVE_COUNT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DecisionPrimitiveId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DecisionRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DecisionPrimitiveKind {
    Glyph,
    Rect,
    Ellipse,
    Arc,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionSourcePrimitive {
    pub id: DecisionPrimitiveId,
    pub kind: DecisionPrimitiveKind,
    pub atlas_entry: Option<u16>,
    pub bounds: DecisionRect,
    pub rgba: [u8; 4],
    pub depth_band: u8,
    pub motion: u8,
    pub dynamic: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionSourceFixture {
    pub schema_version: u16,
    pub id: String,
    pub primitives: Vec<DecisionSourcePrimitive>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionResolvedPrimitive {
    pub id: DecisionPrimitiveId,
    pub kind: DecisionPrimitiveKind,
    pub atlas_entry: Option<u16>,
    pub bounds: DecisionRect,
    pub rgba: [u8; 4],
    pub depth_band: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionResolvedFrame {
    pub frame_index: u64,
    pub elapsed_ms: u64,
    pub primitives: Vec<DecisionResolvedPrimitive>,
    pub changed_primitive_ids: Vec<DecisionPrimitiveId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionExpectedRegion {
    pub name: String,
    pub bounds: DecisionRect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionExpectedFrame {
    pub frame_index: u64,
    pub required_primitive_ids: Vec<DecisionPrimitiveId>,
    pub expected_regions: Vec<DecisionExpectedRegion>,
    pub expected_changes: Vec<DecisionPrimitiveId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionAtlasEntry {
    pub key: String,
    pub rect: [u16; 4],
    pub advance: u16,
    pub baseline: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionAtlas {
    pub schema_version: u16,
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
    pub entries: Vec<DecisionAtlasEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionSemanticNode {
    pub id: String,
    pub role: String,
    pub name: String,
    pub value: Option<String>,
    pub parent: Option<String>,
    pub bounds: DecisionRect,
    pub hidden: bool,
    pub focusable: bool,
}

pub fn canonical_fixture() -> DecisionSourceFixture {
    let mut primitives = Vec::with_capacity(PET_GLYPH_COUNT + STATIC_GLYPH_COUNT + SHAPE_COUNT);
    for index in 0..PET_GLYPH_COUNT {
        primitives.push(glyph_primitive(index, true));
    }
    for index in 0..STATIC_GLYPH_COUNT {
        primitives.push(glyph_primitive(PET_GLYPH_COUNT + index, false));
    }
    for index in 0..SHAPE_COUNT {
        let id = PET_GLYPH_COUNT + STATIC_GLYPH_COUNT + index;
        primitives.push(DecisionSourcePrimitive {
            id: DecisionPrimitiveId(id as u16),
            kind: match index % 3 {
                0 => DecisionPrimitiveKind::Rect,
                1 => DecisionPrimitiveKind::Ellipse,
                _ => DecisionPrimitiveKind::Arc,
            },
            atlas_entry: None,
            bounds: DecisionRect {
                x: 16.0 + (index % 10) as f32 * 31.0,
                y: 18.0 + (index / 10) as f32 * 72.0,
                width: 18.0 + (index % 4) as f32,
                height: 10.0 + (index % 5) as f32,
            },
            rgba: [40 + (index % 40) as u8, 90, 120, 180],
            depth_band: (index % 3) as u8,
            motion: (index % 4) as u8,
            dynamic: false,
        });
    }
    DecisionSourceFixture {
        schema_version: FIXTURE_SCHEMA_VERSION,
        id: FIXTURE_ID.to_string(),
        primitives,
    }
}

fn glyph_primitive(index: usize, pet: bool) -> DecisionSourcePrimitive {
    let dynamic = pet && index < DYNAMIC_PRIMITIVE_COUNT;
    let local_index = if pet { index } else { index - PET_GLYPH_COUNT };
    DecisionSourcePrimitive {
        id: DecisionPrimitiveId(index as u16),
        kind: DecisionPrimitiveKind::Glyph,
        atlas_entry: Some((local_index % 16) as u16),
        bounds: DecisionRect {
            x: if pet { 82.0 } else { 26.0 } + (local_index % 20) as f32 * 10.0,
            y: if pet { 100.0 } else { 28.0 } + (local_index / 20) as f32 * 12.0,
            width: 9.0,
            height: 11.0,
        },
        rgba: if pet {
            [126, 220, 188, 255]
        } else {
            [82, 132, 148, 255]
        },
        depth_band: (local_index % 3) as u8,
        motion: (local_index % 4) as u8,
        dynamic,
    }
}

pub fn resolve_frame(fixture: &DecisionSourceFixture, elapsed_ms: u64) -> DecisionResolvedFrame {
    let semantic_tick = elapsed_ms / 250;
    let phase = elapsed_ms as f32 / 1000.0;
    let mut changed_primitive_ids = Vec::new();
    let primitives = fixture
        .primitives
        .iter()
        .map(|source| {
            let mut bounds = source.bounds;
            match source.motion {
                0 => bounds.y += phase.sin() * 2.0,
                1 => bounds.x += (phase * 0.7).cos() * 1.5,
                2 => {
                    bounds.x += phase.sin();
                    bounds.y += phase.cos();
                }
                _ => bounds.y += (phase * 0.5).sin() * 0.5,
            }
            let mut atlas_entry = source.atlas_entry;
            if source.dynamic {
                atlas_entry = Some(((source.id.0 as u64 + semantic_tick) % 16) as u16);
                if semantic_tick > 0 {
                    changed_primitive_ids.push(source.id);
                }
            }
            DecisionResolvedPrimitive {
                id: source.id,
                kind: source.kind,
                atlas_entry,
                bounds,
                rgba: source.rgba,
                depth_band: source.depth_band,
            }
        })
        .collect();
    DecisionResolvedFrame {
        frame_index: elapsed_ms / 66,
        elapsed_ms,
        primitives,
        changed_primitive_ids,
    }
}

pub fn expected_frame(fixture: &DecisionSourceFixture, elapsed_ms: u64) -> DecisionExpectedFrame {
    let frame = resolve_frame(fixture, elapsed_ms);
    DecisionExpectedFrame {
        frame_index: frame.frame_index,
        required_primitive_ids: fixture
            .primitives
            .iter()
            .map(|primitive| primitive.id)
            .collect(),
        expected_regions: vec![DecisionExpectedRegion {
            name: "round-aperture".to_string(),
            bounds: DecisionRect {
                x: 0.0,
                y: 0.0,
                width: 360.0,
                height: 360.0,
            },
        }],
        expected_changes: frame.changed_primitive_ids,
    }
}

pub fn canonical_atlas() -> DecisionAtlas {
    const CELL: u16 = 8;
    const COLS: u16 = 4;
    const ROWS: u16 = 4;
    let width = CELL * COLS;
    let height = CELL * ROWS;
    let mut rgba = vec![0_u8; usize::from(width) * usize::from(height) * 4];
    let mut entries = Vec::new();
    let keys = [
        "@", "#", "%", "&", "*", "+", "-", ".", "o", "O", "x", "X", "?", "�", "🫧", "o\u{308}",
    ];
    for (index, key) in keys.into_iter().enumerate() {
        let col = index as u16 % COLS;
        let row = index as u16 / COLS;
        for y in 1..CELL - 1 {
            for x in 1..CELL - 1 {
                let px = col * CELL + x;
                let py = row * CELL + y;
                let offset = (usize::from(py) * usize::from(width) + usize::from(px)) * 4;
                let on = !(x + y + index as u16).is_multiple_of(3);
                rgba[offset..offset + 4].copy_from_slice(if on {
                    &[255, 255, 255, 255]
                } else {
                    &[0, 0, 0, 0]
                });
            }
        }
        entries.push(DecisionAtlasEntry {
            key: key.to_string(),
            rect: [col * CELL, row * CELL, CELL, CELL],
            advance: CELL,
            baseline: 6,
        });
    }
    DecisionAtlas {
        schema_version: 1,
        width,
        height,
        rgba,
        entries,
    }
}

pub fn semantic_fixture(logical_size: u16, hidden: bool) -> Vec<DecisionSemanticNode> {
    let scale = f32::from(logical_size) / 360.0;
    vec![
        DecisionSemanticNode {
            id: "habitat".into(),
            role: "group".into(),
            name: "Glorp habitat".into(),
            value: None,
            parent: None,
            bounds: DecisionRect {
                x: 0.0,
                y: 0.0,
                width: f32::from(logical_size),
                height: f32::from(logical_size),
            },
            hidden,
            focusable: false,
        },
        semantic_value("today", "Today", "1.2B", 88.0 * scale, hidden),
        semantic_value(
            "pace",
            "Pace",
            "25.4M per ten minutes",
            174.0 * scale,
            hidden,
        ),
        semantic_value(
            "comparison",
            "Daily comparison",
            "52 percent yesterday",
            260.0 * scale,
            hidden,
        ),
    ]
}

fn semantic_value(id: &str, name: &str, value: &str, y: f32, hidden: bool) -> DecisionSemanticNode {
    let scale = y / match id {
        "today" => 88.0,
        "pace" => 174.0,
        "comparison" => 260.0,
        _ => 1.0,
    };
    DecisionSemanticNode {
        id: id.into(),
        role: "static-text".into(),
        name: name.into(),
        value: Some(value.into()),
        parent: Some("habitat".into()),
        bounds: DecisionRect {
            x: 80.0 * scale,
            y,
            width: 200.0 * scale,
            height: 24.0 * scale,
        },
        hidden,
        focusable: false,
    }
}
