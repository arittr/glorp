use super::DepthCue;
use crate::presentation::privacy::PrivacyProjection;
use std::fmt;
use std::ops::Mul;
use std::sync::Arc;

pub const SCENE_CONTRACT_SCHEMA_VERSION: u16 = super::COMPANION_SCENE_SCHEMA_VERSION;
pub const MAX_SCENE_NODES: usize = 128;
pub const MAX_STATIC_PRIMITIVES: usize = 768;
pub const MAX_PET_ART_SLOTS: usize = 130;
pub const MAX_ROOM_GLYPH_SLOTS: usize = 32;
pub const MAX_VISIBLE_PROPS: usize = 10;
pub const MAX_ROUND_TANK_INHABITANTS: usize = 2;
pub const MAX_AMBIENT_INSTANCES: usize = 64;
pub const MAX_BLENDED_DRAWS: usize = 256;
pub const MAX_LIGHTS: usize = 2;
pub const MAX_ATTACHMENTS: usize = 32;
pub const MAX_PROP_GLYPHS_PER_SLOT: usize = 9;
pub const MAX_TANK_GLYPHS_PER_SLOT: usize = 8;
pub const MAX_STATIC_ATLAS_RECIPES: usize = 8;
pub const MAX_ANALYTIC_PARAMS: usize = 16;
/// Forward renderer-neutral paint for the closed ambient mote family. The
/// legacy companion currently emits no visible motes, but future neutral
/// projections must use this source rather than borrowing a TUI color.
pub const AMBIENT_MOTE_COLOR_SRGB8: [u8; 3] = [0xb8, 0xd4, 0xec];
pub const LIT_CARD_SCALE_TOLERANCE: f32 = 1.0e-5;
pub const MIN_LIT_CARD_WORLD_SCALE: f64 = 1.0e-6;
// V1 emits SDR, but these bounds leave ample headroom for authored HDR-like
// key/rim values while keeping normalization and two-light shader math finite.
pub const MIN_LIGHT_DIRECTION_NORM: f64 = 1.0e-6;
pub const MAX_LIGHT_DIRECTION_NORM: f64 = 1_024.0;
pub const MAX_LIGHT_COLOR_LINEAR: f32 = 16.0;
pub const MAX_LIGHT_INTENSITY: f32 = 64.0;
pub const MAX_LIGHT_COLOR_INTENSITY_PRODUCT: f64 = 256.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliasError {
    Empty,
    NonCanonicalAscii,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalAlias(String);

impl fmt::Debug for CanonicalAlias {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CanonicalAlias(<redacted>)")
    }
}

impl CanonicalAlias {
    pub fn new(alias: impl Into<String>) -> Result<Self, AliasError> {
        let alias = alias.into();
        if alias.is_empty() {
            return Err(AliasError::Empty);
        }
        let mut previous_was_separator = false;
        for (index, byte) in alias.bytes().enumerate() {
            let is_separator = matches!(byte, b'.' | b'-' | b'_');
            let is_canonical = byte.is_ascii_lowercase() || byte.is_ascii_digit() || is_separator;
            if !is_canonical
                || (is_separator && (index == 0 || index + 1 == alias.len()))
                || (is_separator && previous_was_separator)
            {
                return Err(AliasError::NonCanonicalAscii);
            }
            previous_was_separator = is_separator;
        }
        Ok(Self(alias))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn fnv1a_32(alias: &CanonicalAlias) -> u32 {
    alias.as_str().bytes().fold(0x811c_9dc5, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

macro_rules! semantic_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
        #[serde(transparent)]
        pub struct $name(pub u32);

        impl $name {
            pub fn from_alias(alias: &CanonicalAlias) -> Self {
                Self(fnv1a_32(alias))
            }
        }
    };
}

semantic_id!(NodeId);
semantic_id!(AttachmentId);
semantic_id!(MaterialId);
semantic_id!(ResourceId);
semantic_id!(StaticAtlasSourceKey);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct StaticAtlasRecipeId(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct AnalyticParamId(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrimitiveKind {
    AtlasQuad,
    AnalyticShape,
    ShallowCard,
    InstanceQuad,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MaterialKind {
    UnlitGlyphSprite,
    UnlitAnalytic,
    LitShallowCard,
    MultiplyShadow,
    AdditiveGlow,
    ScreenChrome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorldBlend {
    Opaque,
    AlphaCutout,
    PremultipliedAlpha,
    Multiply,
    Additive,
}

/// Screen chrome has its own fixed phase and does not consume the world blend
/// stream. A missing material is conservatively classified as world content;
/// reference validation reports the dangling material separately.
pub fn is_world_blended(blend: WorldBlend, material: Option<MaterialKind>) -> bool {
    matches!(
        blend,
        WorldBlend::PremultipliedAlpha | WorldBlend::Multiply | WorldBlend::Additive
    ) && material != Some(MaterialKind::ScreenChrome)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttachmentMode {
    Follow,
    SnapshotWorldOnSpawn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DepthBehavior {
    WorldWrite,
    WorldReadOnly,
    ScreenNoDepth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKind {
    GlyphAtlas,
    ColorAtlas,
    AnalyticGeometry,
    ShallowCardGeometry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformError {
    NonFinite,
    ZeroQuaternion,
}

/// A column-major matrix applied to column vectors (`clip = matrix * point`).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Mat4 {
    pub columns: [[f32; 4]; 4],
}

impl Mat4 {
    pub const IDENTITY: Self = Self {
        columns: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    pub fn transform_point3(self, point: [f32; 3]) -> [f32; 4] {
        self.transform_vector4([point[0], point[1], point[2], 1.0])
    }

    pub fn transform_vector4(self, vector: [f32; 4]) -> [f32; 4] {
        let mut output = [0.0; 4];
        for (row, output_value) in output.iter_mut().enumerate() {
            *output_value = self.columns[0][row] * vector[0]
                + self.columns[1][row] * vector[1]
                + self.columns[2][row] * vector[2]
                + self.columns[3][row] * vector[3];
        }
        output
    }

    fn translation(value: [f32; 3]) -> Self {
        let mut matrix = Self::IDENTITY;
        matrix.columns[3][0] = value[0];
        matrix.columns[3][1] = value[1];
        matrix.columns[3][2] = value[2];
        matrix
    }

    fn scale(value: [f32; 3]) -> Self {
        Self {
            columns: [
                [value[0], 0.0, 0.0, 0.0],
                [0.0, value[1], 0.0, 0.0],
                [0.0, 0.0, value[2], 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    fn active_quaternion_xyzw(value: [f32; 4]) -> Result<Self, TransformError> {
        if !value.iter().all(|component| component.is_finite()) {
            return Err(TransformError::NonFinite);
        }
        let length_squared = value
            .iter()
            .map(|component| f64::from(*component) * f64::from(*component))
            .sum::<f64>();
        if length_squared == 0.0 || !length_squared.is_finite() {
            return Err(TransformError::ZeroQuaternion);
        }
        let inverse_length = length_squared.sqrt().recip();
        let [x, y, z, w] = value.map(|component| (f64::from(component) * inverse_length) as f32);
        Ok(Self {
            columns: [
                [
                    1.0 - 2.0 * (y * y + z * z),
                    2.0 * (x * y + z * w),
                    2.0 * (x * z - y * w),
                    0.0,
                ],
                [
                    2.0 * (x * y - z * w),
                    1.0 - 2.0 * (x * x + z * z),
                    2.0 * (y * z + x * w),
                    0.0,
                ],
                [
                    2.0 * (x * z + y * w),
                    2.0 * (y * z - x * w),
                    1.0 - 2.0 * (x * x + y * y),
                    0.0,
                ],
                [0.0, 0.0, 0.0, 1.0],
            ],
        })
    }
}

impl Mul for Mat4 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let mut columns = [[0.0; 4]; 4];
        for (column_index, column) in columns.iter_mut().enumerate() {
            *column = self.transform_vector4(rhs.columns[column_index]);
        }
        Self { columns }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Transform3 {
    pub translation: [f32; 3],
    pub rotation_xyzw: [f32; 4],
    pub scale: [f32; 3],
    pub pivot: [f32; 3],
}

impl Transform3 {
    pub const IDENTITY: Self = Self {
        translation: [0.0; 3],
        rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0; 3],
        pivot: [0.0; 3],
    };

    pub const fn translated(translation: [f32; 3]) -> Self {
        Self { translation, ..Self::IDENTITY }
    }

    pub fn from_snapshot_xy_depth(position: [f32; 3], layout_height_points: f32) -> Self {
        Self::translated([position[0], layout_height_points - position[1], position[2]])
    }

    pub fn matrix(self) -> Result<Mat4, TransformError> {
        if !self
            .translation
            .iter()
            .chain(self.scale.iter())
            .chain(self.pivot.iter())
            .all(|value| value.is_finite())
        {
            return Err(TransformError::NonFinite);
        }
        let rotation = Mat4::active_quaternion_xyzw(self.rotation_xyzw)?;
        let negative_pivot = self.pivot.map(|value| -value);
        Ok(Mat4::translation(self.translation)
            * Mat4::translation(self.pivot)
            * rotation
            * Mat4::scale(self.scale)
            * Mat4::translation(negative_pivot))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraError {
    NonFinite,
    InvalidExtent,
    InvalidDepthRange,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct OrthographicCamera {
    pub width_points: f32,
    pub height_points: f32,
    pub far_z: f32,
    pub near_z: f32,
}

impl OrthographicCamera {
    pub fn new(
        width_points: f32,
        height_points: f32,
        far_z: f32,
        near_z: f32,
    ) -> Result<Self, CameraError> {
        if ![width_points, height_points, far_z, near_z]
            .iter()
            .all(|value| value.is_finite())
        {
            return Err(CameraError::NonFinite);
        }
        if width_points <= 0.0 || height_points <= 0.0 {
            return Err(CameraError::InvalidExtent);
        }
        if near_z <= far_z {
            return Err(CameraError::InvalidDepthRange);
        }
        let camera = Self {
            width_points,
            height_points,
            far_z,
            near_z,
        };
        camera.projection_matrix()?;
        Ok(camera)
    }

    pub fn clip_depth(self, world_z: f32) -> Result<f32, CameraError> {
        if !world_z.is_finite() {
            return Err(CameraError::NonFinite);
        }
        let clip = (self.near_z - world_z) / (self.near_z - self.far_z);
        clip.is_finite()
            .then_some(clip)
            .ok_or(CameraError::InvalidDepthRange)
    }

    pub fn projection_matrix(self) -> Result<Mat4, CameraError> {
        let depth_range = self.near_z - self.far_z;
        if !depth_range.is_finite() || depth_range <= 0.0 {
            return Err(CameraError::InvalidDepthRange);
        }
        let matrix = Mat4 {
            columns: [
                [2.0 / self.width_points, 0.0, 0.0, 0.0],
                [0.0, 2.0 / self.height_points, 0.0, 0.0],
                [0.0, 0.0, -1.0 / depth_range, 0.0],
                [-1.0, -1.0, self.near_z / depth_range, 1.0],
            ],
        };
        matrix
            .columns
            .iter()
            .flatten()
            .all(|value| value.is_finite())
            .then_some(matrix)
            .ok_or(CameraError::InvalidExtent)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Bounds3 {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeTemplate {
    pub id: NodeId,
    pub alias: CanonicalAlias,
    pub parent: Option<NodeId>,
    pub base_transform: Transform3,
    pub local_bounds: Bounds3,
    pub depth_cue: DepthCue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaterialTemplate {
    pub id: MaterialId,
    pub alias: CanonicalAlias,
    pub kind: MaterialKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceTemplate {
    pub id: ResourceId,
    pub alias: CanonicalAlias,
    pub kind: ResourceKind,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PrimitiveTemplate {
    pub node: NodeId,
    pub kind: PrimitiveKind,
    pub material: MaterialId,
    pub resource: Option<ResourceId>,
    pub blend: WorldBlend,
    pub depth: DepthBehavior,
    pub binding: PrimitiveBinding,
    pub authored_order: u16,
    pub local_geometry: Bounds3,
    pub space: PrimitiveSpace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrimitiveSpace {
    World,
    Screen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstanceLayer {
    Behind,
    Foreground,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstanceGroupBinding {
    RoomGlyphs,
    PetArt(PetArtFilter),
    PropGlyphs(u8),
    TankCells { slot: u8, layer: InstanceLayer },
    Ambient,
    Hud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PetArtFilter {
    Body,
    Particles,
}

impl PetArtFilter {
    pub const fn includes(self, role: PetPaletteRole) -> bool {
        match self {
            Self::Body => !matches!(role, PetPaletteRole::Particle),
            Self::Particles => matches!(role, PetPaletteRole::Particle),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrimitiveBinding {
    ShallowCard,
    Instances(InstanceGroupBinding),
    Analytic(AnalyticParamId),
    StaticAtlas(StaticAtlasRecipeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttachmentInstanceBinding {
    PropGlyphs(u8),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttachmentTemplate {
    pub id: AttachmentId,
    pub alias: CanonicalAlias,
    pub owner: NodeId,
    pub local: Transform3,
    pub mode: AttachmentMode,
    pub instance_binding: Option<AttachmentInstanceBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentResolveError {
    DanglingOwner,
    MissingFrameNode,
    MissingInstanceSource,
    AmbiguousInstanceSource,
    OwnerOutsideInstanceSource,
    SlotOutOfBounds,
    InvalidTransform,
    HierarchyCycle,
}

pub fn resolve_attachment_world(
    template: &SceneTemplate,
    frame: &SceneFrame,
    attachment: &AttachmentTemplate,
) -> Result<Mat4, AttachmentResolveError> {
    let source = match attachment.instance_binding {
        None => None,
        Some(AttachmentInstanceBinding::PropGlyphs(slot)) => {
            if usize::from(slot) >= frame.prop_slots.len() {
                return Err(AttachmentResolveError::SlotOutOfBounds);
            }
            let mut matches = template
                .primitives
                .iter()
                .filter(|primitive| {
                    primitive.binding
                        == PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(slot))
                })
                .map(|primitive| primitive.node);
            let source = matches
                .next()
                .ok_or(AttachmentResolveError::MissingInstanceSource)?;
            if matches.next().is_some() {
                return Err(AttachmentResolveError::AmbiguousInstanceSource);
            }
            Some((source, slot))
        }
    };

    let mut chain = [NodeId(0); MAX_SCENE_NODES];
    let mut count = 0;
    let mut current = Some(attachment.owner);
    while let Some(id) = current {
        if count >= chain.len() || chain[..count].contains(&id) {
            return Err(AttachmentResolveError::HierarchyCycle);
        }
        chain[count] = id;
        count += 1;
        let node = template
            .nodes
            .iter()
            .find(|node| node.id == id)
            .ok_or(AttachmentResolveError::DanglingOwner)?;
        current = node.parent;
    }
    if source.is_some_and(|(source, _)| !chain[..count].contains(&source)) {
        return Err(AttachmentResolveError::OwnerOutsideInstanceSource);
    }

    let mut world = Mat4::IDENTITY;
    for id in chain[..count].iter().rev().copied() {
        let node = template
            .nodes
            .iter()
            .find(|node| node.id == id)
            .ok_or(AttachmentResolveError::DanglingOwner)?;
        let dynamic = frame
            .nodes
            .iter()
            .find(|node| node.node == id)
            .ok_or(AttachmentResolveError::MissingFrameNode)?;
        world = world
            * node
                .base_transform
                .matrix()
                .map_err(|_| AttachmentResolveError::InvalidTransform)?
            * dynamic
                .local_transform
                .matrix()
                .map_err(|_| AttachmentResolveError::InvalidTransform)?;
        if let Some((source, slot)) = source {
            if source == id {
                let prop = frame.prop_slots[usize::from(slot)];
                world = world
                    * Transform3::translated([
                        prop.origin_points[0] + prop.motion_offset_points[0],
                        prop.origin_points[1] + prop.motion_offset_points[1],
                        0.0,
                    ])
                    .matrix()
                    .map_err(|_| AttachmentResolveError::InvalidTransform)?;
            }
        }
    }
    Ok(world
        * attachment
            .local
            .matrix()
            .map_err(|_| AttachmentResolveError::InvalidTransform)?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct SceneCapacities {
    pub max_nodes: usize,
    pub max_static_primitives: usize,
    pub max_pet_art_slots: usize,
    pub max_room_glyph_slots: usize,
    pub max_visible_props: usize,
    pub max_round_tank_inhabitants: usize,
    pub max_ambient_instances: usize,
    pub max_blended_draws: usize,
    pub max_lights: usize,
    pub max_attachments: usize,
    pub max_static_atlas_recipes: usize,
    pub max_analytic_params: usize,
}

impl SceneCapacities {
    pub const FIXED_V2: Self = Self {
        max_nodes: MAX_SCENE_NODES,
        max_static_primitives: MAX_STATIC_PRIMITIVES,
        max_pet_art_slots: MAX_PET_ART_SLOTS,
        max_room_glyph_slots: MAX_ROOM_GLYPH_SLOTS,
        max_visible_props: MAX_VISIBLE_PROPS,
        max_round_tank_inhabitants: MAX_ROUND_TANK_INHABITANTS,
        max_ambient_instances: MAX_AMBIENT_INSTANCES,
        max_blended_draws: MAX_BLENDED_DRAWS,
        max_lights: MAX_LIGHTS,
        max_attachments: MAX_ATTACHMENTS,
        max_static_atlas_recipes: MAX_STATIC_ATLAS_RECIPES,
        max_analytic_params: MAX_ANALYTIC_PARAMS,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StaticAtlasSemantic {
    DecorativeSprite,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct StaticAtlasRecipe {
    pub semantic: StaticAtlasSemantic,
    pub source: StaticAtlasSourceKey,
    pub local_bounds: Bounds3,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct StaticAtlasRecipeSlot {
    pub id: StaticAtlasRecipeId,
    pub recipe: Option<StaticAtlasRecipe>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnalyticSemantic {
    RoomBackground,
    WallShadow,
    FloorProjection,
    StatusHalo,
    MoodAura,
    Gauges,
    Trouble,
    Dim,
}

impl AnalyticSemantic {
    pub const ALL: [Self; 8] = [
        Self::RoomBackground,
        Self::WallShadow,
        Self::FloorProjection,
        Self::StatusHalo,
        Self::MoodAura,
        Self::Gauges,
        Self::Trouble,
        Self::Dim,
    ];

    pub const fn id(self) -> AnalyticParamId {
        AnalyticParamId(match self {
            Self::RoomBackground => 0,
            Self::WallShadow => 1,
            Self::FloorProjection => 2,
            Self::StatusHalo => 3,
            Self::MoodAura => 4,
            Self::Gauges => 5,
            Self::Trouble => 6,
            Self::Dim => 7,
        })
    }

    pub const fn shape(self) -> AnalyticShape {
        match self {
            Self::RoomBackground => AnalyticShape::ApertureRadial,
            Self::WallShadow => AnalyticShape::PetSilhouette,
            Self::FloorProjection => AnalyticShape::RadialEllipse,
            Self::StatusHalo => AnalyticShape::StatusBeacon,
            Self::MoodAura => AnalyticShape::PetAura,
            Self::Gauges => AnalyticShape::PerimeterGaugeSet,
            Self::Trouble => AnalyticShape::TroubleBeacon,
            Self::Dim => AnalyticShape::SurfaceOverlay,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnalyticShape {
    ApertureRadial,
    PetSilhouette,
    RadialEllipse,
    StatusBeacon,
    PetAura,
    PerimeterGaugeSet,
    TroubleBeacon,
    SurfaceOverlay,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct AnalyticTemplate {
    pub semantic: AnalyticSemantic,
    pub shape: AnalyticShape,
    /// Canonical normalized local geometry. Dynamic point-space placement is
    /// deliberately owned by the later analytic frame contract.
    pub normalized_local_bounds: Bounds3,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct AnalyticTemplateSlot {
    pub id: AnalyticParamId,
    pub value: Option<AnalyticTemplate>,
}

fn empty_static_atlas_recipe_slots() -> Vec<StaticAtlasRecipeSlot> {
    (0..MAX_STATIC_ATLAS_RECIPES)
        .map(|slot| StaticAtlasRecipeSlot {
            id: StaticAtlasRecipeId(slot as u8),
            recipe: None,
        })
        .collect()
}

fn empty_analytic_template_slots() -> Vec<AnalyticTemplateSlot> {
    (0..MAX_ANALYTIC_PARAMS)
        .map(|slot| AnalyticTemplateSlot {
            id: AnalyticParamId(slot as u8),
            value: None,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneTemplate {
    pub schema_version: u16,
    pub renderer_schema_version: u16,
    pub capacities: SceneCapacities,
    pub glyph_grid: super::CompanionGlyphGrid,
    pub nodes: Vec<NodeTemplate>,
    pub primitives: Vec<PrimitiveTemplate>,
    pub materials: Vec<MaterialTemplate>,
    pub resources: Vec<ResourceTemplate>,
    pub attachments: Vec<AttachmentTemplate>,
    pub static_atlas_recipes: Vec<StaticAtlasRecipeSlot>,
    pub analytic_templates: Vec<AnalyticTemplateSlot>,
    pub privacy: PrivacyProjection,
    pub generation_checksum: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentValueError {
    InvalidPetGlyph,
    InvalidAuthoredGlyph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct PetGlyph(char);

impl PetGlyph {
    pub fn for_species(
        glyph: char,
        species: crate::pet::generation::Species,
    ) -> Result<Self, ContentValueError> {
        crate::pet::render::declared_pet_glyphs(species)
            .contains(&glyph)
            .then_some(Self(glyph))
            .ok_or(ContentValueError::InvalidPetGlyph)
    }

    pub const fn as_char(self) -> char {
        self.0
    }
}

/// A glyph from the closed companion-authored repertoire. This deliberately
/// excludes arbitrary user text and control characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct AuthoredGlyph(char);

impl AuthoredGlyph {
    pub fn new(glyph: char) -> Result<Self, ContentValueError> {
        const REPERTOIRE: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz.,:;!?%+-_#*/\\()[]<>|~`'\"@=&^$◆◇◈◉○◌◦◡◑◔◜▲▼◣◢▝▴▱▂▃▓▣☁☼✦✧✺·•∘°˚˙‹›ѱ⁙⌁⌞⌟╭╮╰╯╲╱╵╷╽╿┃│┊─┄╌┬~≈□";
        REPERTOIRE
            .chars()
            .any(|candidate| candidate == glyph)
            .then_some(Self(glyph))
            .ok_or(ContentValueError::InvalidAuthoredGlyph)
    }

    pub const fn as_char(self) -> char {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PetPaletteRole {
    Body,
    BodyGlow,
    Eye,
    Mouth,
    Accent,
    Pattern,
    Particle,
    Corruption,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AmbientContentKind {
    Mote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MoodContentKind {
    Happy,
    Ecstatic,
    Content,
    Hungry,
    Sad,
    Sleepy,
    Wilted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WeatherContentKind {
    Clear,
    CacheMist,
    OutputSparks,
    ReasoningPulse,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PropGlyphContent {
    pub glyph: Option<AuthoredGlyph>,
    pub local_cell: [i8; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PropSemanticContent {
    pub sprite_phase: Option<u8>,
    pub twinkle_active: Option<bool>,
    pub lid_open: Option<bool>,
    pub bloom_active: Option<bool>,
    pub glyphs: [PropGlyphContent; MAX_PROP_GLYPHS_PER_SLOT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct TankSemanticContent {
    pub sprite_variant: u8,
    pub morph: Option<u8>,
    pub color_srgb8: [u8; 3],
    pub bold: bool,
    pub glyphs: [Option<AuthoredGlyph>; MAX_TANK_GLYPHS_PER_SLOT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PetArtSlot {
    pub slot: u16,
    pub glyph: Option<PetGlyph>,
    pub palette_role: PetPaletteRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct RoomGlyphContentSlot {
    pub slot: u8,
    pub glyph: Option<AuthoredGlyph>,
    pub color_srgb8: Option<[u8; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PropContentSlot {
    pub slot: u8,
    pub content: Option<PropSemanticContent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct TankContentSlot {
    pub slot: u8,
    pub content: Option<TankSemanticContent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AmbientContentSlot {
    pub slot: u8,
    pub kind: Option<AmbientContentKind>,
    pub glyph: Option<AuthoredGlyph>,
}

/// Renderer-neutral glyph paint. Keeping this as a closed sRGB source makes
/// the retained GPU mirror independent of both terminal colors and shader
/// implementation details.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct GlyphPaintSource {
    pub color_srgb8: [u8; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PropGlyphPaintSlot {
    pub slot: u8,
    pub paints: [Option<GlyphPaintSource>; MAX_PROP_GLYPHS_PER_SLOT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AmbientGlyphPaintSlot {
    pub slot: u8,
    pub paint: Option<GlyphPaintSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct GaugeLanePaint {
    pub track_srgba8: [u8; 4],
    pub fill_srgba8: [u8; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnalyticPaint {
    ApertureDepth {
        core_srgb8: [u8; 3],
        rim_srgb8: [u8; 3],
    },
    PetShadowMultiply {
        color_srgb8: [u8; 3],
        opacity_u8: u8,
    },
    FloorShadowMultiplyRadial {
        inner_srgba8: [u8; 4],
        outer_srgba8: [u8; 4],
    },
    StatusBeacon {
        active_srgba8: [u8; 4],
        calm_srgba8: [u8; 4],
    },
    MoodAuraRings {
        color_srgb8: [u8; 3],
        ring_count: u8,
        per_ring_alpha_u8: u8,
    },
    PerimeterGaugeSet {
        xp: GaugeLanePaint,
        daily: GaugeLanePaint,
        pace: GaugeLanePaint,
        daily_overage_srgba8: [u8; 4],
    },
    TroubleBeacon {
        color_srgba8: [u8; 4],
    },
    DimOverlay {
        color_srgb8: [u8; 3],
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AnalyticContent {
    pub semantic: AnalyticSemantic,
    pub shape: AnalyticShape,
    pub paint: AnalyticPaint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AnalyticContentSlot {
    pub id: AnalyticParamId,
    pub value: Option<AnalyticContent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyticMaskSource {
    PetBody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBeaconTone {
    Active,
    Calm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GaugeLineCap {
    Round,
}

#[derive(Clone, Copy, PartialEq)]
pub struct GaugeLaneGeometry {
    pub radius_points: f32,
    pub stroke_width_points: f32,
    pub track_start_degrees: f32,
    pub track_sweep_degrees: f32,
    pub cap: GaugeLineCap,
}

/// Closed geometry payloads for the eight companion analytic roles. Exact
/// gauges, activity opacity, and dim amount remain in their existing private
/// frame fields; these records only describe renderer-neutral geometry.
#[derive(Clone, Copy, PartialEq)]
pub enum AnalyticGeometry {
    ApertureRadial {
        center_points: [f32; 2],
        radius_points: f32,
        feather_points: f32,
    },
    PetSilhouette {
        mask: AnalyticMaskSource,
        offset_points: [f32; 2],
        softness_points: f32,
    },
    RadialEllipse {
        center_points: [f32; 2],
        radii_points: [f32; 2],
        softness_points: f32,
    },
    StatusBeacon {
        center_points: [f32; 2],
        radius_points: f32,
        thickness_points: f32,
        tone: StatusBeaconTone,
    },
    PetAura {
        center_points: [f32; 2],
        max_radius_points: f32,
        ring_count: u8,
        feather_points: f32,
    },
    PerimeterGaugeSet {
        center_points: [f32; 2],
        xp: GaugeLaneGeometry,
        daily: GaugeLaneGeometry,
        pace: GaugeLaneGeometry,
    },
    TroubleBeacon {
        center_points: [f32; 2],
        radius_points: f32,
        thickness_points: f32,
    },
    SurfaceOverlay,
}

#[derive(Clone, Copy, PartialEq)]
pub struct AnalyticFrame {
    pub semantic: AnalyticSemantic,
    pub shape: AnalyticShape,
    /// Y-up point-space `[x, y, width, height]` bounds.
    pub rect_points: [f32; 4],
    pub geometry: AnalyticGeometry,
}

#[derive(Clone, Copy, PartialEq)]
pub struct AnalyticFrameSlot {
    pub id: AnalyticParamId,
    pub value: Option<AnalyticFrame>,
}

fn empty_analytic_content_slots() -> Vec<AnalyticContentSlot> {
    (0..MAX_ANALYTIC_PARAMS)
        .map(|slot| AnalyticContentSlot {
            id: AnalyticParamId(slot as u8),
            value: None,
        })
        .collect()
}

fn empty_analytic_frame_slots() -> Vec<AnalyticFrameSlot> {
    (0..MAX_ANALYTIC_PARAMS)
        .map(|slot| AnalyticFrameSlot {
            id: AnalyticParamId(slot as u8),
            value: None,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneContent {
    pub schema_version: u16,
    pub renderer_schema_version: u16,
    pub palette: [[u8; 3]; 8],
    pub mood: MoodContentKind,
    pub weather: WeatherContentKind,
    pub day_phase: super::CompanionDayPhase,
    pub pet_art_slots: Vec<PetArtSlot>,
    pub room_glyph_slots: Vec<RoomGlyphContentSlot>,
    pub prop_slots: Vec<PropContentSlot>,
    pub tank_slots: Vec<TankContentSlot>,
    pub ambient_slots: Vec<AmbientContentSlot>,
    pub prop_paint_slots: Vec<PropGlyphPaintSlot>,
    pub ambient_paint_slots: Vec<AmbientGlyphPaintSlot>,
    pub analytic_slots: Vec<AnalyticContentSlot>,
}

const fn zero_generation_key() -> super::SceneGenerationKey {
    super::SceneGenerationKey {
        device: super::DeviceEpoch(0),
        layout: super::LayoutGeneration(0),
        resources: super::ResourceGeneration(0),
    }
}

impl SceneContent {
    pub fn empty_v2() -> Self {
        Self {
            schema_version: SCENE_CONTRACT_SCHEMA_VERSION,
            renderer_schema_version: super::COMPANION_RENDERER_SCHEMA_VERSION,
            palette: [[0; 3]; 8],
            mood: MoodContentKind::Content,
            weather: WeatherContentKind::Clear,
            day_phase: super::CompanionDayPhase::Day,
            pet_art_slots: (0..MAX_PET_ART_SLOTS)
                .map(|slot| PetArtSlot {
                    slot: slot as u16,
                    glyph: None,
                    palette_role: PetPaletteRole::Body,
                })
                .collect(),
            room_glyph_slots: (0..MAX_ROOM_GLYPH_SLOTS)
                .map(|slot| RoomGlyphContentSlot {
                    slot: slot as u8,
                    glyph: None,
                    color_srgb8: None,
                })
                .collect(),
            prop_slots: (0..MAX_VISIBLE_PROPS)
                .map(|slot| PropContentSlot { slot: slot as u8, content: None })
                .collect(),
            tank_slots: (0..MAX_ROUND_TANK_INHABITANTS)
                .map(|slot| TankContentSlot { slot: slot as u8, content: None })
                .collect(),
            ambient_slots: (0..MAX_AMBIENT_INSTANCES)
                .map(|slot| AmbientContentSlot {
                    slot: slot as u8,
                    kind: None,
                    glyph: None,
                })
                .collect(),
            prop_paint_slots: (0..MAX_VISIBLE_PROPS)
                .map(|slot| PropGlyphPaintSlot {
                    slot: slot as u8,
                    paints: [None; MAX_PROP_GLYPHS_PER_SLOT],
                })
                .collect(),
            ambient_paint_slots: (0..MAX_AMBIENT_INSTANCES)
                .map(|slot| AmbientGlyphPaintSlot { slot: slot as u8, paint: None })
                .collect(),
            analytic_slots: empty_analytic_content_slots(),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct NodeFrameState {
    pub node: NodeId,
    pub local_transform: Transform3,
    pub visible: bool,
    pub opacity: f32,
}

impl fmt::Debug for NodeFrameState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NodeFrameState")
            .field("node", &self.node)
            .field("local_transform", &self.local_transform)
            .field("visible", &self.visible)
            .field("opacity", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct LightFrame {
    pub direction: [f32; 3],
    pub color_linear: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct PropFrameSlot {
    pub slot: u8,
    pub visible: bool,
    pub origin_points: [f32; 2],
    pub motion_offset_points: [f32; 2],
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct TankCellFrame {
    pub visible: bool,
    pub position_points: [f32; 2],
    pub layer: InstanceLayer,
    pub bounds_points: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct TankFrameSlot {
    pub slot: u8,
    pub visible: bool,
    pub origin_points: [f32; 2],
    pub cells: [TankCellFrame; MAX_TANK_GLYPHS_PER_SLOT],
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct AmbientFrameSlot {
    pub slot: u8,
    pub visible: bool,
    pub position_points: [f32; 2],
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct RoomGlyphFrameSlot {
    pub slot: u8,
    pub visible: bool,
    pub grid_cell: [u16; 2],
    pub position_points: [f32; 2],
    pub opacity: f32,
}

#[derive(Clone, PartialEq)]
pub struct SceneFrame {
    pub schema_version: u16,
    pub renderer_schema_version: u16,
    pub camera: OrthographicCamera,
    pub nodes: Vec<NodeFrameState>,
    pub room_glyph_slots: Vec<RoomGlyphFrameSlot>,
    pub prop_slots: Vec<PropFrameSlot>,
    pub tank_slots: Vec<TankFrameSlot>,
    pub ambient_slots: Vec<AmbientFrameSlot>,
    pub analytic_slots: Vec<AnalyticFrameSlot>,
    pub gauges: [f32; 4],
    pub dim_amount: f32,
    pub lights: Vec<LightFrame>,
}

impl SceneFrame {
    pub fn empty_v2(camera: OrthographicCamera) -> Self {
        let hidden_tank_cell = TankCellFrame {
            visible: false,
            position_points: [0.0; 2],
            layer: InstanceLayer::Behind,
            bounds_points: [0.0; 4],
        };
        Self {
            schema_version: SCENE_CONTRACT_SCHEMA_VERSION,
            renderer_schema_version: super::COMPANION_RENDERER_SCHEMA_VERSION,
            camera,
            nodes: Vec::new(),
            room_glyph_slots: (0..MAX_ROOM_GLYPH_SLOTS)
                .map(|slot| RoomGlyphFrameSlot {
                    slot: slot as u8,
                    visible: false,
                    grid_cell: [0; 2],
                    position_points: [0.0; 2],
                    opacity: 0.0,
                })
                .collect(),
            prop_slots: (0..MAX_VISIBLE_PROPS)
                .map(|slot| PropFrameSlot {
                    slot: slot as u8,
                    visible: false,
                    origin_points: [0.0; 2],
                    motion_offset_points: [0.0; 2],
                    opacity: 0.0,
                })
                .collect(),
            tank_slots: (0..MAX_ROUND_TANK_INHABITANTS)
                .map(|slot| TankFrameSlot {
                    slot: slot as u8,
                    visible: false,
                    origin_points: [0.0; 2],
                    cells: [hidden_tank_cell; MAX_TANK_GLYPHS_PER_SLOT],
                })
                .collect(),
            ambient_slots: (0..MAX_AMBIENT_INSTANCES)
                .map(|slot| AmbientFrameSlot {
                    slot: slot as u8,
                    visible: false,
                    position_points: [0.0; 2],
                    opacity: 0.0,
                })
                .collect(),
            analytic_slots: empty_analytic_frame_slots(),
            gauges: [0.0; 4],
            dim_amount: 0.0,
            lights: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CaptureFramePrivacyProjection {
    gauges: [super::GaugeLevelSnapshot; 4],
    dimmed: bool,
}

impl CaptureFramePrivacyProjection {
    pub(crate) fn from_frame(frame: &SceneFrame) -> Self {
        Self {
            gauges: frame
                .gauges
                .map(|gauge| super::GaugeLevelSnapshot::from_fraction(f64::from(gauge))),
            dimmed: frame.dim_amount > 0.0,
        }
    }

    pub(crate) const fn gauges(self) -> [super::GaugeLevelSnapshot; 4] {
        self.gauges
    }

    pub(crate) const fn dimmed(self) -> bool {
        self.dimmed
    }

    pub(crate) fn node_state(
        self,
        canonical_alias: &str,
        visible: bool,
        opacity: f32,
    ) -> (bool, f32) {
        match canonical_alias {
            "chrome.dim" => (self.dimmed, if self.dimmed { 1.0 } else { 0.0 }),
            "chrome.status" => (visible, if visible { 1.0 } else { 0.0 }),
            _ => (visible, opacity),
        }
    }
}

impl fmt::Debug for SceneFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SceneFrame")
            .field("schema_version", &self.schema_version)
            .field("renderer_schema_version", &self.renderer_schema_version)
            .field("camera", &self.camera)
            .field("node_count", &self.nodes.len())
            .field("room_glyph_slot_count", &self.room_glyph_slots.len())
            .field("prop_slot_count", &self.prop_slots.len())
            .field("tank_slot_count", &self.tank_slots.len())
            .field("ambient_slot_count", &self.ambient_slots.len())
            .field("analytic_slot_count", &self.analytic_slots.len())
            .field("gauges", &"<redacted>")
            .field("dim_amount", &"<redacted>")
            .field("light_count", &self.lights.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContentDelta {
    pub schema_version: u16,
    pub renderer_schema_version: u16,
    pub generation_key: super::SceneGenerationKey,
    pub from: super::AppliedRevisions,
    pub to: super::AppliedRevisions,
    pub palette: Option<[[u8; 3]; 8]>,
    pub mood: Option<MoodContentKind>,
    pub weather: Option<WeatherContentKind>,
    pub day_phase: Option<super::CompanionDayPhase>,
    pub pet_art_slots: Vec<PetArtSlot>,
    pub room_glyph_slots: Vec<RoomGlyphContentSlot>,
    pub prop_slots: Vec<PropContentSlot>,
    pub tank_slots: Vec<TankContentSlot>,
    pub ambient_slots: Vec<AmbientContentSlot>,
    pub prop_paint_slots: Vec<PropGlyphPaintSlot>,
    pub ambient_paint_slots: Vec<AmbientGlyphPaintSlot>,
    pub analytic_slots: Vec<AnalyticContentSlot>,
}

impl ContentDelta {
    pub const fn empty() -> Self {
        Self {
            schema_version: SCENE_CONTRACT_SCHEMA_VERSION,
            renderer_schema_version: super::COMPANION_RENDERER_SCHEMA_VERSION,
            generation_key: zero_generation_key(),
            from: super::AppliedRevisions::new(0, 0),
            to: super::AppliedRevisions::new(0, 0),
            palette: None,
            mood: None,
            weather: None,
            day_phase: None,
            pet_art_slots: Vec::new(),
            room_glyph_slots: Vec::new(),
            prop_slots: Vec::new(),
            tank_slots: Vec::new(),
            ambient_slots: Vec::new(),
            prop_paint_slots: Vec::new(),
            ambient_paint_slots: Vec::new(),
            analytic_slots: Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct FrameDelta {
    pub schema_version: u16,
    pub renderer_schema_version: u16,
    pub generation_key: super::SceneGenerationKey,
    pub from: super::AppliedRevisions,
    pub to: super::AppliedRevisions,
    pub camera: Option<OrthographicCamera>,
    pub nodes: Vec<NodeFrameState>,
    pub room_glyph_slots: Vec<RoomGlyphFrameSlot>,
    pub prop_slots: Vec<PropFrameSlot>,
    pub tank_slots: Vec<TankFrameSlot>,
    pub ambient_slots: Vec<AmbientFrameSlot>,
    pub analytic_slots: Vec<AnalyticFrameSlot>,
    pub gauges: Option<[f32; 4]>,
    pub dim_amount: Option<f32>,
    pub lights: Vec<(u8, LightFrame)>,
}

impl fmt::Debug for FrameDelta {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FrameDelta")
            .field("schema_version", &self.schema_version)
            .field("renderer_schema_version", &self.renderer_schema_version)
            .field("camera", &self.camera)
            .field("node_count", &self.nodes.len())
            .field("room_glyph_slot_count", &self.room_glyph_slots.len())
            .field("prop_slot_count", &self.prop_slots.len())
            .field("tank_slot_count", &self.tank_slots.len())
            .field("ambient_slot_count", &self.ambient_slots.len())
            .field("analytic_slot_count", &self.analytic_slots.len())
            .field("gauges", &self.gauges.map(|_| "<redacted>"))
            .field("dim_amount", &self.dim_amount.map(|_| "<redacted>"))
            .field("light_count", &self.lights.len())
            .finish()
    }
}

impl FrameDelta {
    pub const fn empty() -> Self {
        Self {
            schema_version: SCENE_CONTRACT_SCHEMA_VERSION,
            renderer_schema_version: super::COMPANION_RENDERER_SCHEMA_VERSION,
            generation_key: zero_generation_key(),
            from: super::AppliedRevisions::new(0, 0),
            to: super::AppliedRevisions::new(0, 0),
            camera: None,
            nodes: Vec::new(),
            room_glyph_slots: Vec::new(),
            prop_slots: Vec::new(),
            tank_slots: Vec::new(),
            ambient_slots: Vec::new(),
            analytic_slots: Vec::new(),
            gauges: None,
            dim_amount: None,
            lights: Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct SceneGenerationData {
    generation_key: super::SceneGenerationKey,
    source_revisions: super::AppliedRevisions,
    source_snapshot: Arc<super::CompanionSceneSnapshot>,
    request_seal: Arc<()>,
    template: SceneTemplate,
    content: SceneContent,
    frame: SceneFrame,
    content_checksum: u64,
    frame_checksum: u64,
    pub(crate) delta_scratch: SceneDeltaScratch,
    accepted: super::validate::AcceptedSceneState,
}

impl fmt::Debug for SceneGenerationData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SceneGenerationData")
            .field("generation_key", &self.generation_key)
            .field("source_revisions", &self.source_revisions)
            .field("scene_schema_version", &self.template.schema_version)
            .field(
                "renderer_schema_version",
                &self.template.renderer_schema_version,
            )
            .field("template_checksum", &self.template.generation_checksum)
            .field("content_checksum", &self.content_checksum)
            .field("frame_checksum", &"<redacted>")
            .field(
                "capture_frame_checksum",
                &self.frame.capture_source_checksum(&self.template).ok(),
            )
            .field("node_count", &self.template.nodes.len())
            .field("primitive_count", &self.template.primitives.len())
            .field("light_count", &self.frame.lights.len())
            .field("source_snapshot", &"<redacted exact frame>")
            .field("frame", &"<redacted exact frame>")
            .field("delta_scratch", &"<redacted exact frame deltas>")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SceneDeltaScratch {
    pub content: ContentDelta,
    pub frame: FrameDelta,
}

impl SceneDeltaScratch {
    fn fixed_v2() -> Self {
        Self {
            content: ContentDelta {
                schema_version: SCENE_CONTRACT_SCHEMA_VERSION,
                renderer_schema_version: super::COMPANION_RENDERER_SCHEMA_VERSION,
                generation_key: zero_generation_key(),
                from: super::AppliedRevisions::new(0, 0),
                to: super::AppliedRevisions::new(0, 0),
                palette: None,
                mood: None,
                weather: None,
                day_phase: None,
                pet_art_slots: Vec::with_capacity(MAX_PET_ART_SLOTS),
                room_glyph_slots: Vec::with_capacity(MAX_ROOM_GLYPH_SLOTS),
                prop_slots: Vec::with_capacity(MAX_VISIBLE_PROPS),
                tank_slots: Vec::with_capacity(MAX_ROUND_TANK_INHABITANTS),
                ambient_slots: Vec::with_capacity(MAX_AMBIENT_INSTANCES),
                prop_paint_slots: Vec::with_capacity(MAX_VISIBLE_PROPS),
                ambient_paint_slots: Vec::with_capacity(MAX_AMBIENT_INSTANCES),
                analytic_slots: Vec::with_capacity(MAX_ANALYTIC_PARAMS),
            },
            frame: FrameDelta {
                schema_version: SCENE_CONTRACT_SCHEMA_VERSION,
                renderer_schema_version: super::COMPANION_RENDERER_SCHEMA_VERSION,
                generation_key: zero_generation_key(),
                from: super::AppliedRevisions::new(0, 0),
                to: super::AppliedRevisions::new(0, 0),
                camera: None,
                nodes: Vec::with_capacity(MAX_SCENE_NODES),
                room_glyph_slots: Vec::with_capacity(MAX_ROOM_GLYPH_SLOTS),
                prop_slots: Vec::with_capacity(MAX_VISIBLE_PROPS),
                tank_slots: Vec::with_capacity(MAX_ROUND_TANK_INHABITANTS),
                ambient_slots: Vec::with_capacity(MAX_AMBIENT_INSTANCES),
                analytic_slots: Vec::with_capacity(MAX_ANALYTIC_PARAMS),
                gauges: None,
                dim_amount: None,
                lights: Vec::with_capacity(MAX_LIGHTS),
            },
        }
    }
}

mod checksum;
mod compiler;

#[cfg(test)]
use checksum::canonical_f32_bits;
#[cfg(test)]
use compiler::prop_glyphs;
pub use compiler::{build_scene_generation, SceneGenerationError};
#[allow(unused_imports)] // Public Task 8 seam; Task 5 exercises it only in unit tests.
pub(crate) use compiler::{
    build_scene_generation_for_request, build_scene_generation_owned, SceneDeltaApplyError,
};

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct SceneFixture {
    pub template: SceneTemplate,
    pub content: SceneContent,
    pub frame: SceneFrame,
}

#[cfg(test)]
impl SceneFixture {
    pub fn valid() -> Self {
        fn alias(value: &str) -> CanonicalAlias {
            CanonicalAlias::new(value).unwrap()
        }
        let root_alias = alias("scene.root");
        let child_alias = alias("pet.body");
        let root = NodeId::from_alias(&root_alias);
        let child = NodeId::from_alias(&child_alias);
        let material_alias = alias("material.pet-glyph");
        let material = MaterialId::from_alias(&material_alias);
        let resource_alias = alias("resource.pet-glyph-atlas");
        let resource = ResourceId::from_alias(&resource_alias);
        let atlas_source = StaticAtlasSourceKey::from_alias(&resource_alias);
        let attachment_alias = alias("pet.body.bubble-origin");
        let camera = OrthographicCamera::new(360.0, 360.0, -2.0, 2.0).unwrap();
        let mut static_atlas_recipes = empty_static_atlas_recipe_slots();
        static_atlas_recipes[0].recipe = Some(StaticAtlasRecipe {
            semantic: StaticAtlasSemantic::DecorativeSprite,
            source: atlas_source,
            local_bounds: Bounds3 { min: [0.0; 3], max: [1.0, 1.0, 0.0] },
        });
        Self {
            template: SceneTemplate {
                schema_version: SCENE_CONTRACT_SCHEMA_VERSION,
                renderer_schema_version: super::COMPANION_RENDERER_SCHEMA_VERSION,
                capacities: SceneCapacities::FIXED_V2,
                glyph_grid: super::CompanionGlyphGrid {
                    columns: 30,
                    rows: 30,
                    y_up_origin_points: [0.0, 0.0],
                    cell_extent_points: [12.0, 12.0],
                    scale: super::LogicalGlyphScale::OneCell,
                    anchor: super::LogicalGlyphAnchor::CellBottomLeft,
                },
                nodes: vec![
                    NodeTemplate {
                        id: root,
                        alias: root_alias,
                        parent: None,
                        base_transform: Transform3::IDENTITY,
                        local_bounds: Bounds3 { min: [0.0; 3], max: [360.0, 360.0, 0.0] },
                        depth_cue: DepthCue::NEUTRAL,
                    },
                    NodeTemplate {
                        id: child,
                        alias: child_alias,
                        parent: Some(root),
                        base_transform: Transform3::IDENTITY,
                        local_bounds: Bounds3 { min: [0.0; 3], max: [13.0, 10.0, 0.0] },
                        depth_cue: DepthCue::NEUTRAL,
                    },
                ],
                primitives: vec![PrimitiveTemplate {
                    node: child,
                    kind: PrimitiveKind::AtlasQuad,
                    material,
                    resource: Some(resource),
                    blend: WorldBlend::AlphaCutout,
                    depth: DepthBehavior::WorldWrite,
                    binding: PrimitiveBinding::StaticAtlas(StaticAtlasRecipeId(0)),
                    authored_order: 0,
                    local_geometry: Bounds3 { min: [0.0; 3], max: [1.0, 1.0, 0.0] },
                    space: PrimitiveSpace::World,
                }],
                materials: vec![MaterialTemplate {
                    id: material,
                    alias: material_alias,
                    kind: MaterialKind::UnlitGlyphSprite,
                }],
                resources: vec![ResourceTemplate {
                    id: resource,
                    alias: resource_alias,
                    kind: ResourceKind::GlyphAtlas,
                }],
                attachments: vec![AttachmentTemplate {
                    id: AttachmentId::from_alias(&attachment_alias),
                    alias: attachment_alias,
                    owner: child,
                    local: Transform3::IDENTITY,
                    mode: AttachmentMode::Follow,
                    instance_binding: None,
                }],
                static_atlas_recipes,
                analytic_templates: compiler::build_analytic_templates(Bounds3 {
                    min: [0.0; 3],
                    max: [1.0, 1.0, 0.0],
                }),
                privacy: PrivacyProjection::for_surface(
                    crate::presentation::privacy::PresentationSurface::RoundCompanion,
                ),
                generation_checksum: 1,
            },
            content: SceneContent {
                schema_version: SCENE_CONTRACT_SCHEMA_VERSION,
                renderer_schema_version: super::COMPANION_RENDERER_SCHEMA_VERSION,
                palette: [[0; 3]; 8],
                mood: MoodContentKind::Content,
                weather: WeatherContentKind::Clear,
                day_phase: super::CompanionDayPhase::Day,
                pet_art_slots: (0..MAX_PET_ART_SLOTS)
                    .map(|slot| PetArtSlot {
                        slot: slot as u16,
                        glyph: (slot == 0).then(|| {
                            PetGlyph::for_species('^', crate::pet::generation::Species::Fuzz)
                                .unwrap()
                        }),
                        palette_role: PetPaletteRole::Body,
                    })
                    .collect(),
                room_glyph_slots: SceneContent::empty_v2().room_glyph_slots,
                prop_slots: SceneContent::empty_v2().prop_slots,
                tank_slots: SceneContent::empty_v2().tank_slots,
                ambient_slots: SceneContent::empty_v2().ambient_slots,
                prop_paint_slots: SceneContent::empty_v2().prop_paint_slots,
                ambient_paint_slots: SceneContent::empty_v2().ambient_paint_slots,
                analytic_slots: compiler::build_analytic_content(&SceneContent::empty_v2()),
            },
            frame: SceneFrame {
                schema_version: SCENE_CONTRACT_SCHEMA_VERSION,
                renderer_schema_version: super::COMPANION_RENDERER_SCHEMA_VERSION,
                camera,
                nodes: vec![
                    NodeFrameState {
                        node: root,
                        local_transform: Transform3::IDENTITY,
                        visible: true,
                        opacity: 1.0,
                    },
                    NodeFrameState {
                        node: child,
                        local_transform: Transform3::IDENTITY,
                        visible: true,
                        opacity: 1.0,
                    },
                ],
                room_glyph_slots: SceneFrame::empty_v2(camera).room_glyph_slots,
                prop_slots: SceneFrame::empty_v2(camera).prop_slots,
                tank_slots: SceneFrame::empty_v2(camera).tank_slots,
                ambient_slots: SceneFrame::empty_v2(camera).ambient_slots,
                analytic_slots: compiler::fixture_analytic_frame_slots(camera),
                gauges: [0.0; 4],
                dim_amount: 0.0,
                lights: vec![],
            },
        }
    }

    pub fn valid_lit_card() -> SceneTemplate {
        let mut template = Self::valid().template;
        template.materials[0].kind = MaterialKind::LitShallowCard;
        template.primitives[0].kind = PrimitiveKind::ShallowCard;
        template.primitives[0].binding = PrimitiveBinding::ShallowCard;
        template.resources[0].kind = ResourceKind::ShallowCardGeometry;
        template.primitives[0].blend = WorldBlend::Opaque;
        template
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::evolution::Stage;
    use crate::game::metabolism::Mood;
    use crate::pet::generation::{generate_pet, Species};
    use crate::pet::render::{render_pet, AnimationFrame, PaletteRoleName};
    use crate::presentation::companion_scene::{CompanionLogicalLayout, CompanionSceneSnapshot};
    use crate::presentation::privacy::{PresentationSurface, PrivacyProjection};

    const EPSILON: f32 = 1.0e-5;

    fn generation_key(value: u64) -> super::super::SceneGenerationKey {
        super::super::SceneGenerationKey {
            device: super::super::DeviceEpoch(value),
            layout: super::super::LayoutGeneration(value + 1),
            resources: super::super::ResourceGeneration(value + 2),
        }
    }

    fn set_depth_lifecycle(
        snapshot: &mut super::super::CompanionSceneSnapshot,
        asleep: bool,
        calm: bool,
    ) {
        snapshot.frame.asleep = asleep;
        snapshot.frame.calm = calm || asleep;
        let resolved = crate::round::depth::resolve_smooth_depth(
            snapshot.frame.pet_depth,
            crate::round::depth::depth_lifecycle_scale(snapshot.frame.asleep, snapshot.frame.calm),
        )
        .unwrap();
        snapshot.frame.pet_depth_cue = super::super::DepthCue {
            scale: resolved.scale,
            y_offset_points_up: -resolved.perspective_y
                * snapshot.topology.glyph_grid.cell_extent_points[1],
            opacity: resolved.atmosphere,
            saturation: 1.0,
        };
    }

    fn snapshot_for(species: Species, stage: Stage) -> CompanionSceneSnapshot {
        let generated = generate_pet("direct-unlit-scene-fixture").with_species(species);
        let rendered = render_pet(&generated, stage, Mood::Content, AnimationFrame::default());
        let source_lines = rendered.lines;
        let mut pet_lines = source_lines
            .iter()
            .map(|line| {
                let mut chars = line.chars().collect::<Vec<_>>();
                chars.resize(super::super::PET_LATTICE_WIDTH as usize, ' ');
                chars.into_iter().collect::<String>()
            })
            .collect::<Vec<_>>();
        pet_lines.resize(
            super::super::PET_LATTICE_HEIGHT as usize,
            " ".repeat(super::super::PET_LATTICE_WIDTH as usize),
        );
        let pet_roles = rendered
            .spans
            .iter()
            .map(|span| super::super::PetRoleSpanSnapshot {
                line_index: span.line as u16,
                start_char: span.start as u16,
                end_char: span.end as u16,
                role: match span.role {
                    PaletteRoleName::Body => "body",
                    PaletteRoleName::BodyGlow => "body-glow",
                    PaletteRoleName::Eye => "eye",
                    PaletteRoleName::Mouth => "mouth",
                    PaletteRoleName::Accent => "accent",
                    PaletteRoleName::Pattern => "pattern",
                    PaletteRoleName::Particle => "particle",
                    PaletteRoleName::Corruption => "corruption",
                },
            })
            .collect();
        CompanionSceneSnapshot {
            schema_version: super::super::COMPANION_SCENE_SCHEMA_VERSION,
            privacy: PrivacyProjection::for_surface(PresentationSurface::RoundCompanion),
            topology: super::super::TopologySnapshot {
                layout: CompanionLogicalLayout::round(360.0, 360.0),
                glyph_grid: super::super::CompanionGlyphGrid {
                    columns: 60,
                    rows: 30,
                    y_up_origin_points: [0.0, 0.0],
                    cell_extent_points: [6.0, 12.0],
                    scale: super::super::LogicalGlyphScale::OneCell,
                    anchor: super::super::LogicalGlyphAnchor::CellBottomLeft,
                },
                pet: super::super::PetTopologySnapshot {
                    species,
                    stage,
                    lattice: super::super::PetLatticeSnapshot {
                        identity: "pet-art-13x10-v1",
                        width: super::super::PET_LATTICE_WIDTH,
                        height: super::super::PET_LATTICE_HEIGHT,
                        slot_count: super::super::PET_LATTICE_SLOTS,
                    },
                },
                room: super::super::RoomTopologySnapshot {
                    primary_biome: "starter",
                    secondary_biome: None,
                    species_dialect: species.as_str(),
                },
                visible_props: Vec::new(),
                visible_tank_inhabitants: Vec::new(),
                renderer_schema: super::super::COMPANION_RENDERER_SCHEMA_VERSION,
            },
            content: super::super::ContentSnapshot {
                mood: Mood::Content,
                room_weather: "clear",
                day_phase: super::super::CompanionDayPhase::Day,
                pet_lines,
                pet_roles,
                room_glyphs: Vec::new(),
                palette: super::super::PaletteSnapshot {
                    body: [120, 130, 140],
                    body_glow: [150, 160, 170],
                    eye: [220, 230, 240],
                    mouth: [180, 100, 110],
                    accent: [100, 180, 200],
                    pattern: [90, 100, 110],
                    particle: [200, 190, 100],
                    corruption: [210, 70, 180],
                },
                prop_animation_states: Vec::new(),
                tank_animation_states: Vec::new(),
                ambient_semantics: (0..MAX_AMBIENT_INSTANCES)
                    .map(|slot| super::super::AmbientSemanticSnapshot {
                        slot: slot as u8,
                        kind: None,
                        glyph: None,
                    })
                    .collect(),
            },
            frame: super::super::FrameSnapshot {
                elapsed_ms: 1_000,
                pet_anchor_points: [120.0, 140.0],
                pet_depth: 0.0,
                pet_depth_cue: super::super::DepthCue::NEUTRAL,
                facing: 1,
                breath_offset_y_points: 0.0,
                bob_offset_y_points: 0.0,
                asleep: false,
                calm: false,
                helper_trouble: false,
                activity_recent: false,
                activity_opacity: 0.0,
                gauge_levels: [super::super::GaugeLevelSnapshot::Empty; 4],
                gauge_fractions: [0.0; 4],
                dimmed: false,
                dim_amount: 0.0,
                room_glyphs: Vec::new(),
                ambient_instances: (0..MAX_AMBIENT_INSTANCES)
                    .map(|slot| super::super::AmbientFrameSnapshot {
                        slot: slot as u8,
                        visible: false,
                        position_points: [0.0; 2],
                        opacity: 0.0,
                    })
                    .collect(),
            },
        }
    }

    #[test]
    fn token_lantern_scene_glyphs_stay_in_the_active_species_repertoire() {
        for species in Species::all() {
            let repertoire = crate::round::smooth::collect_companion_glyph_repertoire(
                &crate::round::smooth::CompanionContentIdentity::for_pet(species),
            );
            for twinkle in [false, true] {
                let glyphs = compiler::prop_glyphs(
                    crate::game::habitat::TOKEN_LANTERN_10M,
                    species,
                    Some(0),
                    Some(twinkle),
                    None,
                    None,
                )
                .expect("the token lantern is valid authored scene content");
                for glyph in glyphs.iter().filter_map(|cell| cell.glyph) {
                    let sequence = glyph.as_char().to_string();
                    assert!(
                        repertoire
                            .iter()
                            .any(|declared| !declared.bold && declared.sequence == sequence),
                        "{species:?} token lantern emitted undeclared glyph {sequence:?} with twinkle={twinkle}"
                    );
                }
            }
        }
    }

    fn assert_point_close(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= EPSILON,
                "{actual} != {expected}"
            );
        }
    }

    #[test]
    fn fixed_v2_capacities_are_exact() {
        assert_eq!(MAX_SCENE_NODES, 128);
        assert_eq!(MAX_STATIC_PRIMITIVES, 768);
        assert_eq!(MAX_PET_ART_SLOTS, 130);
        assert_eq!(MAX_ROOM_GLYPH_SLOTS, 32);
        assert_eq!(MAX_VISIBLE_PROPS, 10);
        assert_eq!(MAX_ROUND_TANK_INHABITANTS, 2);
        assert_eq!(MAX_AMBIENT_INSTANCES, 64);
        assert_eq!(MAX_BLENDED_DRAWS, 256);
        assert_eq!(MAX_LIGHTS, 2);
        assert_eq!(MAX_ATTACHMENTS, 32);
        assert_eq!(MAX_PROP_GLYPHS_PER_SLOT, 9);
        assert_eq!(MAX_TANK_GLYPHS_PER_SLOT, 8);
    }

    #[test]
    fn repaired_mirrors_have_canonical_empty_slots_and_fixed_frame_storage() {
        let content = SceneContent::empty_v2();
        assert_eq!(content.analytic_slots.len(), MAX_ANALYTIC_PARAMS);
        assert!(content
            .analytic_slots
            .iter()
            .all(|slot| slot.value.is_none()));
        assert_eq!(content.prop_paint_slots.len(), MAX_VISIBLE_PROPS);
        assert!(content
            .prop_paint_slots
            .iter()
            .all(|slot| slot.paints.iter().all(Option::is_none)));
        assert_eq!(content.ambient_paint_slots.len(), MAX_AMBIENT_INSTANCES);
        assert!(content
            .ambient_paint_slots
            .iter()
            .all(|slot| slot.paint.is_none()));
        assert_eq!(content.pet_art_slots.len(), MAX_PET_ART_SLOTS);
        assert_eq!(content.room_glyph_slots.len(), MAX_ROOM_GLYPH_SLOTS);
        assert_eq!(content.prop_slots.len(), MAX_VISIBLE_PROPS);
        assert_eq!(content.tank_slots.len(), MAX_ROUND_TANK_INHABITANTS);
        assert_eq!(content.ambient_slots.len(), MAX_AMBIENT_INSTANCES);
        assert!(content.prop_slots.iter().all(|slot| slot.content.is_none()));
        assert!(content.tank_slots.iter().all(|slot| slot.content.is_none()));
        assert!(content.ambient_slots.iter().all(|slot| slot.kind.is_none()));
        assert!(content
            .room_glyph_slots
            .iter()
            .all(|slot| slot.glyph.is_none()));

        let frame = SceneFrame::empty_v2(OrthographicCamera::new(360.0, 360.0, -2.0, 2.0).unwrap());
        assert_eq!(frame.analytic_slots.len(), MAX_ANALYTIC_PARAMS);
        assert!(frame.analytic_slots.iter().all(|slot| slot.value.is_none()));
        assert_eq!(frame.prop_slots.len(), MAX_VISIBLE_PROPS);
        assert_eq!(frame.room_glyph_slots.len(), MAX_ROOM_GLYPH_SLOTS);
        assert_eq!(frame.tank_slots.len(), MAX_ROUND_TANK_INHABITANTS);
        assert_eq!(frame.ambient_slots.len(), MAX_AMBIENT_INSTANCES);
        assert!(frame.prop_slots.iter().all(|slot| !slot.visible));
        assert!(frame
            .tank_slots
            .iter()
            .flat_map(|slot| slot.cells)
            .all(|cell| !cell.visible));
    }

    #[test]
    fn non_pet_glyphs_are_finite_and_instance_bindings_are_explicit() {
        assert_eq!(AuthoredGlyph::new('◆').unwrap().as_char(), '◆');
        assert_eq!(
            AuthoredGlyph::new('\n'),
            Err(ContentValueError::InvalidAuthoredGlyph)
        );
        assert_eq!(
            AuthoredGlyph::new('\u{1f4a5}'),
            Err(ContentValueError::InvalidAuthoredGlyph)
        );

        let primitive = PrimitiveTemplate {
            node: NodeId(1),
            kind: PrimitiveKind::InstanceQuad,
            material: MaterialId(2),
            resource: Some(ResourceId(3)),
            blend: WorldBlend::AlphaCutout,
            depth: DepthBehavior::WorldWrite,
            binding: PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(4)),
            authored_order: 17,
            local_geometry: Bounds3 { min: [0.0; 3], max: [1.0, 1.0, 0.0] },
            space: PrimitiveSpace::World,
        };
        assert_eq!(
            primitive.binding,
            PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(4))
        );
        assert_eq!(primitive.authored_order, 17);
        assert_eq!(primitive.space, PrimitiveSpace::World);
    }

    #[test]
    fn canonical_checksums_repeat_exclude_runtime_identity_and_normalize_zero() {
        assert_eq!(
            canonical_f32_bits(0.0).unwrap(),
            canonical_f32_bits(-0.0).unwrap()
        );
        assert!(canonical_f32_bits(f32::NAN).is_err());

        let snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        let first = build_scene_generation(&snapshot, generation_key(1)).unwrap();
        let repeated = build_scene_generation(&snapshot, generation_key(1)).unwrap();
        let other_runtime = build_scene_generation(&snapshot, generation_key(99)).unwrap();
        assert_ne!(first.template.generation_checksum, 0);
        assert_eq!(
            first.accepted.template().template().generation_checksum,
            first.template.generation_checksum
        );
        assert_eq!(
            first.template.generation_checksum,
            repeated.template.generation_checksum
        );
        assert_eq!(first.content_checksum, repeated.content_checksum);
        assert_eq!(first.frame_checksum, repeated.frame_checksum);
        assert_eq!(
            first.template.generation_checksum,
            other_runtime.template.generation_checksum
        );
        assert_eq!(first.content_checksum, other_runtime.content_checksum);
        assert_eq!(first.frame_checksum, other_runtime.frame_checksum);
        assert_ne!(first.generation_key, other_runtime.generation_key);

        let mut changed = snapshot.clone();
        changed.content.palette.body[0] ^= 1;
        let changed = build_scene_generation(&changed, generation_key(1)).unwrap();
        assert_ne!(first.content_checksum, changed.content_checksum);
        assert_eq!(
            first.template.generation_checksum,
            changed.template.generation_checksum
        );

        let mut changed = snapshot.clone();
        changed.content.ambient_semantics[0].kind =
            Some(super::super::AmbientSemanticKindSnapshot::Mote);
        changed.content.ambient_semantics[0].glyph = Some('◇');
        let changed = build_scene_generation(&changed, generation_key(1)).unwrap();
        assert_ne!(first.content_checksum, changed.content_checksum);

        let mut changed = snapshot.clone();
        changed.frame.pet_anchor_points[0] += 1.0;
        let changed = build_scene_generation(&changed, generation_key(1)).unwrap();
        assert_ne!(first.frame_checksum, changed.frame_checksum);

        let mut changed = snapshot.clone();
        changed.topology.room.species_dialect = "glitch";
        let changed = build_scene_generation(&changed, generation_key(1)).unwrap();
        assert_ne!(
            first.template.generation_checksum,
            changed.template.generation_checksum
        );
    }

    #[test]
    fn full_fixture_builds_complete_unlit_shallow_hierarchy() {
        let snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        let built = build_scene_generation(&snapshot, generation_key(1)).unwrap();
        assert_eq!(
            built
                .template
                .nodes
                .iter()
                .find(|node| node.alias.as_str() == "scene.root")
                .unwrap()
                .parent,
            None
        );
        assert_eq!(built.content.pet_art_slots.len(), 130);
        assert_eq!(built.content.prop_slots.len(), 10);
        assert_eq!(built.content.tank_slots.len(), 2);
        assert_eq!(built.content.ambient_slots.len(), 64);
        assert!(built
            .template
            .nodes
            .iter()
            .all(|node| node.depth_cue == DepthCue::NEUTRAL));
        assert!(built
            .template
            .primitives
            .iter()
            .all(|primitive| primitive.kind != PrimitiveKind::ShallowCard));
        assert!(built
            .template
            .materials
            .iter()
            .all(|material| material.kind != MaterialKind::LitShallowCard));
        assert!(built.frame.lights.is_empty());
        assert!(built
            .template
            .attachments
            .iter()
            .all(|attachment| attachment.mode == AttachmentMode::Follow));
        super::super::validate::validate_full_generation(
            &built.template,
            &built.content,
            &built.frame,
        )
        .unwrap();
    }

    #[test]
    fn every_species_stage_uses_exact_pet_roles_and_fixed_lattice() {
        let stages = [
            Stage::S0,
            Stage::S1,
            Stage::S2,
            Stage::S3,
            Stage::S4,
            Stage::S5,
            Stage::S6,
        ];
        for species in Species::all() {
            for stage in stages {
                let snapshot = snapshot_for(species, stage);
                let built = build_scene_generation(&snapshot, generation_key(1)).unwrap();
                assert_eq!(
                    built.content.pet_art_slots.len(),
                    130,
                    "{species:?} {stage:?}"
                );
                assert!(built
                    .content
                    .pet_art_slots
                    .iter()
                    .enumerate()
                    .all(|(index, slot)| usize::from(slot.slot) == index));
                let pet_node = built
                    .template
                    .nodes
                    .iter()
                    .find(|node| node.alias.as_str() == "pet")
                    .unwrap();
                let pet_transform = built
                    .frame
                    .nodes
                    .iter()
                    .find(|node| node.node == pet_node.id)
                    .unwrap()
                    .local_transform;
                let (body_center, body_radii) =
                    super::compiler::pet_body_world_geometry(&snapshot, pet_transform).unwrap();
                let aura = built.frame.analytic_slots[4].value.unwrap();
                assert!(
                    aura.geometry
                        == AnalyticGeometry::PetAura {
                            center_points: body_center,
                            max_radius_points:
                                crate::presentation::companion_effects::mood_aura_radius(f64::from(
                                    body_radii[0] * 2.0
                                ),) as f32,
                            ring_count: 8,
                            feather_points: 4.0,
                        },
                    "{species:?} {stage:?}"
                );
            }
        }
    }

    #[test]
    fn compatible_changes_project_through_stable_preallocated_scratch() {
        let snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        let mut built = build_scene_generation(&snapshot, generation_key(1)).unwrap();
        let capacities = built.delta_capacities();
        let storage_pointers = built.delta_storage_pointers();

        let mut facing = snapshot.clone();
        facing.frame.facing *= -1;
        let facing = std::sync::Arc::new(facing);
        let changes = super::super::runtime::classify_snapshot_changes(&snapshot, &facing);
        let deltas = built
            .project_snapshot_changes(
                &facing,
                changes,
                super::super::AppliedRevisions::new(0, 0),
                super::super::AppliedRevisions::new(0, 1),
            )
            .unwrap();
        assert!(deltas.content.pet_art_slots.is_empty());
        assert_eq!(deltas.frame.nodes.len(), 1);
        assert_eq!(built.delta_capacities(), capacities);
        assert_eq!(built.delta_storage_pointers(), storage_pointers);

        let mut raw_clock = snapshot.clone();
        raw_clock.frame.elapsed_ms += 1;
        let raw_clock = std::sync::Arc::new(raw_clock);
        let changes = super::super::runtime::classify_snapshot_changes(&snapshot, &raw_clock);
        let deltas = built
            .project_snapshot_changes(
                &raw_clock,
                changes,
                super::super::AppliedRevisions::new(0, 0),
                super::super::AppliedRevisions::new(0, 0),
            )
            .unwrap();
        assert!(deltas.content.pet_art_slots.is_empty());
        assert!(deltas.frame.nodes.is_empty());
        assert_eq!(built.delta_capacities(), capacities);
        assert_eq!(built.delta_storage_pointers(), storage_pointers);

        let mut mood = snapshot.clone();
        mood.content.mood = Mood::Happy;
        let mood = std::sync::Arc::new(mood);
        let changes = super::super::runtime::classify_snapshot_changes(&snapshot, &mood);
        let deltas = built
            .project_snapshot_changes(
                &mood,
                changes,
                super::super::AppliedRevisions::new(0, 0),
                super::super::AppliedRevisions::new(1, 0),
            )
            .unwrap();
        assert_eq!(deltas.content.mood, Some(MoodContentKind::Happy));
        assert!(
            deltas.content.ambient_slots.is_empty(),
            "mood/weather changes must not republish ambient slots"
        );
        assert_eq!(built.delta_capacities(), capacities);
        assert_eq!(built.delta_storage_pointers(), storage_pointers);
    }

    #[test]
    fn compatible_apply_is_revision_bound_transactional_and_matches_rebuild() {
        let mut initial = snapshot_for(Species::Fuzz, Stage::S3);
        initial.topology.visible_props = vec![super::super::PropTopologySnapshot {
            catalog_id: crate::game::habitat::TOKEN_PEBBLE_25K,
            stable_order: 0,
            zone: super::super::PropZoneSnapshot::FloorMid,
            authored_depth: super::super::AuthoredDepthSnapshot::BehindPet,
        }];
        initial.content.prop_animation_states = vec![super::super::PropAnimationSnapshot {
            catalog_id: crate::game::habitat::TOKEN_PEBBLE_25K,
            stable_order: 0,
            kind: super::super::PropAnimationKindSnapshot::Static,
            sprite_phase: None,
            twinkle_active: None,
            motion_phase: None,
            chest_lid_open: None,
            bloom_active: None,
            origin_points: [180.0, 280.0],
        }];
        let initial = std::sync::Arc::new(initial);
        let mut built = build_scene_generation_owned(
            std::sync::Arc::clone(&initial),
            generation_key(1),
            super::super::AppliedRevisions::new(3, 5),
        )
        .unwrap();
        let mut asleep = (*initial).clone();
        set_depth_lifecycle(&mut asleep, true, true);
        let asleep = std::sync::Arc::new(asleep);
        let changes = super::super::runtime::classify_snapshot_changes(&initial, &asleep);
        let before = built.clone();
        assert_eq!(
            built.apply_compatible_snapshot(
                std::sync::Arc::clone(&asleep),
                changes,
                super::super::AppliedRevisions::new(3, 5),
                super::super::AppliedRevisions::new(2, 6),
            ),
            Err(SceneDeltaApplyError::IdentityMismatch)
        );
        assert_eq!(built, before);
        assert_eq!(
            built.apply_compatible_snapshot(
                std::sync::Arc::clone(&asleep),
                changes,
                super::super::AppliedRevisions::new(3, 5),
                super::super::AppliedRevisions::new(3, 5),
            ),
            Err(SceneDeltaApplyError::IdentityMismatch)
        );
        assert_eq!(built, before);
        built
            .apply_compatible_snapshot(
                std::sync::Arc::clone(&asleep),
                changes,
                super::super::AppliedRevisions::new(3, 5),
                super::super::AppliedRevisions::new(3, 6),
            )
            .unwrap();
        let rebuilt = build_scene_generation_owned(
            std::sync::Arc::clone(&asleep),
            generation_key(1),
            super::super::AppliedRevisions::new(3, 6),
        )
        .unwrap();
        assert_eq!(built.content_checksum, rebuilt.content_checksum);
        assert_eq!(built.frame_checksum, rebuilt.frame_checksum);
        assert_eq!(built.frame.prop_slots, rebuilt.frame.prop_slots);
        assert_eq!(built.frame.prop_slots[0].opacity, 0.72);

        let before = built.clone();
        assert_eq!(
            built.apply_compatible_snapshot(
                std::sync::Arc::clone(&initial),
                super::super::runtime::classify_snapshot_changes(&asleep, &initial),
                super::super::AppliedRevisions::new(3, 5),
                super::super::AppliedRevisions::new(3, 7),
            ),
            Err(SceneDeltaApplyError::StaleBase)
        );
        assert_eq!(built, before);

        let mut different_generation = (*asleep).clone();
        different_generation.topology.pet.stage = Stage::S4;
        let different_generation = std::sync::Arc::new(different_generation);
        let before = built.clone();
        assert_eq!(
            built.apply_compatible_snapshot(
                std::sync::Arc::clone(&different_generation),
                super::super::runtime::classify_snapshot_changes(&asleep, &different_generation,),
                super::super::AppliedRevisions::new(3, 6),
                super::super::AppliedRevisions::new(3, 6),
            ),
            Err(SceneDeltaApplyError::GenerationRequired)
        );
        assert_eq!(built, before);
    }

    #[test]
    fn room_slot_delta_matches_fresh_compile_without_generation_churn() {
        let initial = std::sync::Arc::new(snapshot_for(Species::Fuzz, Stage::S3));
        let mut incremental = build_scene_generation_owned(
            std::sync::Arc::clone(&initial),
            generation_key(7),
            super::super::AppliedRevisions::new(0, 0),
        )
        .unwrap();
        let template_checksum = incremental.template.generation_checksum;
        let mut changed = (*initial).clone();
        changed
            .content
            .room_glyphs
            .push(super::super::RoomGlyphContentSnapshot {
                slot: 0,
                glyph: '✦',
                color_srgb8: [20, 40, 60],
            });
        changed
            .frame
            .room_glyphs
            .push(super::super::RoomGlyphFrameSnapshot {
                slot: 0,
                visible: true,
                grid_cell: [2, 3],
                position_points: [12.0, 312.0],
                opacity: 1.0,
            });
        let changed = std::sync::Arc::new(changed);
        let changes = super::super::runtime::classify_snapshot_changes(&initial, &changed);
        incremental
            .apply_compatible_snapshot(
                std::sync::Arc::clone(&changed),
                changes,
                super::super::AppliedRevisions::new(0, 0),
                super::super::AppliedRevisions::new(1, 1),
            )
            .unwrap();
        let fresh = build_scene_generation_owned(
            std::sync::Arc::clone(&changed),
            generation_key(7),
            super::super::AppliedRevisions::new(1, 1),
        )
        .unwrap();
        assert_eq!(incremental.template.generation_checksum, template_checksum);
        assert_eq!(incremental.template, fresh.template);
        assert_eq!(incremental.content, fresh.content);
        assert_eq!(incremental.frame, fresh.frame);
        assert_eq!(incremental.content_checksum, fresh.content_checksum);
        assert_eq!(incremental.frame_checksum, fresh.frame_checksum);

        let clearing = super::super::runtime::classify_snapshot_changes(&changed, &initial);
        incremental
            .apply_compatible_snapshot(
                std::sync::Arc::clone(&initial),
                clearing,
                super::super::AppliedRevisions::new(1, 1),
                super::super::AppliedRevisions::new(2, 2),
            )
            .unwrap();
        assert!(incremental
            .content
            .room_glyph_slots
            .iter()
            .all(|slot| slot.glyph.is_none() && slot.color_srgb8.is_none()));
        assert!(incremental.frame.room_glyph_slots.iter().all(|slot| {
            !slot.visible
                && slot.grid_cell == [0; 2]
                && slot.position_points == [0.0; 2]
                && slot.opacity == 0.0
        }));
    }

    #[test]
    fn room_authored_identity_changes_template_resources_and_checksum() {
        let snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        let first = build_scene_generation(&snapshot, generation_key(1)).unwrap();
        let mut changed = snapshot.clone();
        changed.topology.room.primary_biome = "technical";
        changed.topology.room.secondary_biome = Some("artifact");
        changed.topology.room.species_dialect = "glitch";
        let changed = build_scene_generation(&changed, generation_key(1)).unwrap();
        assert_ne!(
            first.template.generation_checksum,
            changed.template.generation_checksum
        );
        assert_ne!(first.template.resources, changed.template.resources);
    }

    #[test]
    fn valid_grid_change_is_immutable_template_generation_identity() {
        let initial = snapshot_for(Species::Fuzz, Stage::S3);
        let first = build_scene_generation(&initial, generation_key(1)).unwrap();
        let mut changed = initial.clone();
        changed.topology.glyph_grid.columns = 72;
        changed.topology.glyph_grid.cell_extent_points[0] = 5.0;
        let changes = super::super::runtime::classify_snapshot_changes(&initial, &changed);
        assert!(changes.requires_generation());
        let second = build_scene_generation(&changed, generation_key(2)).unwrap();
        assert_ne!(first.template.glyph_grid, second.template.glyph_grid);
        assert_ne!(
            first.template.generation_checksum,
            second.template.generation_checksum
        );
    }

    #[test]
    fn builder_rejects_mismatched_and_out_of_order_slots_without_panicking() {
        let snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        let mut mismatch = snapshot.clone();
        mismatch
            .content
            .prop_animation_states
            .push(super::super::PropAnimationSnapshot {
                catalog_id: crate::game::habitat::TOKEN_PEBBLE_25K,
                stable_order: 0,
                kind: super::super::PropAnimationKindSnapshot::Static,
                sprite_phase: None,
                twinkle_active: None,
                motion_phase: None,
                chest_lid_open: None,
                bloom_active: None,
                origin_points: [0.0; 2],
            });
        assert!(build_scene_generation(&mismatch, generation_key(1)).is_err());

        let mut bad_slot = snapshot;
        bad_slot.content.ambient_semantics[0].slot = 63;
        assert!(build_scene_generation(&bad_slot, generation_key(1)).is_err());
    }

    #[test]
    fn pet_body_and_particles_share_slots_with_complementary_filters() {
        let built =
            build_scene_generation(&snapshot_for(Species::Fuzz, Stage::S3), generation_key(1))
                .unwrap();
        let bindings = built
            .template
            .primitives
            .iter()
            .filter_map(|primitive| match primitive.binding {
                PrimitiveBinding::Instances(binding) => Some(binding),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(bindings.contains(&InstanceGroupBinding::PetArt(PetArtFilter::Body)));
        assert!(bindings.contains(&InstanceGroupBinding::PetArt(PetArtFilter::Particles)));
        for role in [
            PetPaletteRole::Body,
            PetPaletteRole::BodyGlow,
            PetPaletteRole::Eye,
            PetPaletteRole::Mouth,
            PetPaletteRole::Accent,
            PetPaletteRole::Pattern,
            PetPaletteRole::Particle,
            PetPaletteRole::Corruption,
        ] {
            assert_ne!(
                PetArtFilter::Body.includes(role),
                PetArtFilter::Particles.includes(role)
            );
        }
    }

    #[test]
    fn prop_catalog_states_fit_nine_glyphs_and_chest_attachment_is_lid_owned() {
        for spec in crate::game::habitat::HABITAT_PROP_CATALOG {
            for phase in [Some(0), Some(1)] {
                for active in [Some(false), Some(true)] {
                    let glyphs =
                        prop_glyphs(spec.id, Species::Fuzz, phase, active, active, active).unwrap();
                    assert!(
                        glyphs.iter().any(|slot| slot.glyph.is_some()),
                        "{}",
                        spec.id
                    );
                    assert!(glyphs.iter().filter(|slot| slot.glyph.is_some()).count() <= 9);
                }
            }
        }

        let mut snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        snapshot.topology.visible_props = vec![super::super::PropTopologySnapshot {
            catalog_id: crate::game::habitat::TOKEN_TREASURE_CHEST_2M,
            stable_order: 0,
            zone: super::super::PropZoneSnapshot::FloorMid,
            authored_depth: super::super::AuthoredDepthSnapshot::BehindPet,
        }];
        snapshot.content.prop_animation_states = vec![super::super::PropAnimationSnapshot {
            catalog_id: crate::game::habitat::TOKEN_TREASURE_CHEST_2M,
            stable_order: 0,
            kind: super::super::PropAnimationKindSnapshot::Animated,
            sprite_phase: None,
            twinkle_active: None,
            motion_phase: None,
            chest_lid_open: Some(true),
            bloom_active: None,
            origin_points: [180.0, 280.0],
        }];
        let built = build_scene_generation(&snapshot, generation_key(1)).unwrap();
        let base = crate::game::habitat::catalog_prop_by_str(
            crate::game::habitat::TOKEN_TREASURE_CHEST_2M,
        )
        .unwrap()
        .color;
        let expected = [base.0, base.1, base.2];
        for (glyph, paint) in built.content().prop_slots[0]
            .content
            .unwrap()
            .glyphs
            .into_iter()
            .zip(built.content().prop_paint_slots[0].paints)
        {
            assert_eq!(
                paint.map(|paint| paint.color_srgb8),
                glyph.glyph.map(|_| expected)
            );
        }
        assert_eq!(built.template.attachments.len(), 1);
        let attachment = &built.template.attachments[0];
        let owner = built
            .template
            .nodes
            .iter()
            .find(|node| node.id == attachment.owner)
            .unwrap();
        assert_eq!(
            owner.alias.as_str(),
            "world.prop.token_treasure_chest_2m.lid"
        );
        assert_eq!(
            attachment.alias.as_str(),
            "world.prop.token_treasure_chest_2m.bubble-origin"
        );
        assert_eq!(
            attachment.instance_binding,
            Some(AttachmentInstanceBinding::PropGlyphs(0))
        );
        let resolved = resolve_attachment_world(&built.template, &built.frame, attachment).unwrap();
        assert_point_close(
            resolved.transform_point3([0.0; 3]),
            [180.0, 81.25, -1.6, 1.0],
        );

        let initial = std::sync::Arc::new(snapshot);
        let mut built = build_scene_generation_owned(
            std::sync::Arc::clone(&initial),
            generation_key(1),
            super::super::AppliedRevisions::new(0, 0),
        )
        .unwrap();
        let mut moved = (*initial).clone();
        moved.content.prop_animation_states[0].origin_points[0] += 5.0;
        let moved = std::sync::Arc::new(moved);
        let changes = super::super::runtime::classify_snapshot_changes(&initial, &moved);
        built
            .apply_compatible_snapshot(
                moved,
                changes,
                super::super::AppliedRevisions::new(0, 0),
                super::super::AppliedRevisions::new(0, 1),
            )
            .unwrap();
        let attachment = &built.template.attachments[0];
        let resolved = resolve_attachment_world(&built.template, &built.frame, attachment).unwrap();
        assert_point_close(
            resolved.transform_point3([0.0; 3]),
            [185.0, 81.25, -1.6, 1.0],
        );
    }

    #[test]
    fn retained_prop_motion_uses_authored_cell_scale_and_direction() {
        let offset_for = |catalog_id, motion_phase| {
            let mut snapshot = snapshot_for(Species::Fuzz, Stage::S3);
            snapshot.topology.visible_props = vec![super::super::PropTopologySnapshot {
                catalog_id,
                stable_order: 0,
                zone: super::super::PropZoneSnapshot::FloorMid,
                authored_depth: super::super::AuthoredDepthSnapshot::BehindPet,
            }];
            snapshot.content.prop_animation_states = vec![super::super::PropAnimationSnapshot {
                catalog_id,
                stable_order: 0,
                kind: super::super::PropAnimationKindSnapshot::Animated,
                sprite_phase: None,
                twinkle_active: None,
                motion_phase: Some(motion_phase),
                chest_lid_open: None,
                bloom_active: None,
                origin_points: [180.0, 280.0],
            }];
            let cell_extent = snapshot.topology.glyph_grid.cell_extent_points;
            let built = build_scene_generation(&snapshot, generation_key(1)).unwrap();
            (built.frame.prop_slots[0].motion_offset_points, cell_extent)
        };

        let (pebble_rest, cell) = offset_for(crate::game::habitat::TOKEN_PEBBLE_25K, 0);
        let (pebble_lift, _) = offset_for(crate::game::habitat::TOKEN_PEBBLE_25K, 1);
        let (orbit_rest, _) = offset_for(crate::game::habitat::TOKEN_ORBIT_5M, 0);
        let (orbit_shift, _) = offset_for(crate::game::habitat::TOKEN_ORBIT_5M, 1);
        let (lantern_lift, _) = offset_for(crate::game::habitat::TOKEN_LANTERN_10M, 0);
        let (lantern_rest, _) = offset_for(crate::game::habitat::TOKEN_LANTERN_10M, 1);

        assert_eq!(pebble_rest, [0.0, 0.0]);
        assert_eq!(pebble_lift, [0.0, cell[1]]);
        assert_eq!(orbit_rest, [0.0, 0.0]);
        assert_eq!(orbit_shift, [cell[0], 0.0]);
        assert_eq!(lantern_lift, [0.0, cell[1]]);
        assert_eq!(lantern_rest, [0.0, 0.0]);
    }

    #[test]
    fn occupied_ambient_motes_use_the_forward_neutral_paint_source() {
        let mut snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        snapshot.content.ambient_semantics[0].kind =
            Some(super::super::AmbientSemanticKindSnapshot::Mote);
        snapshot.content.ambient_semantics[0].glyph = Some('◇');
        let built = build_scene_generation(&snapshot, generation_key(42)).unwrap();
        assert_eq!(
            built.content().ambient_paint_slots[0].paint,
            Some(GlyphPaintSource { color_srgb8: AMBIENT_MOTE_COLOR_SRGB8 })
        );
        assert!(built.content().ambient_paint_slots[1..]
            .iter()
            .all(|slot| slot.paint.is_none()));
    }

    #[test]
    fn blooming_prop_uses_closed_blossom_override_without_recoloring_other_cells() {
        let mut snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        snapshot.topology.visible_props = vec![super::super::PropTopologySnapshot {
            catalog_id: crate::game::habitat::HEAVY_SESSION_PLANTER,
            stable_order: 0,
            zone: super::super::PropZoneSnapshot::FloorMid,
            authored_depth: super::super::AuthoredDepthSnapshot::BehindPet,
        }];
        snapshot.content.prop_animation_states = vec![super::super::PropAnimationSnapshot {
            catalog_id: crate::game::habitat::HEAVY_SESSION_PLANTER,
            stable_order: 0,
            kind: super::super::PropAnimationKindSnapshot::Animated,
            sprite_phase: None,
            twinkle_active: None,
            motion_phase: None,
            chest_lid_open: None,
            bloom_active: Some(true),
            origin_points: [180.0, 280.0],
        }];
        let built = build_scene_generation(&snapshot, generation_key(44)).unwrap();
        let content = built.content().prop_slots[0].content.unwrap();
        let paints = built.content().prop_paint_slots[0].paints;
        let spec =
            crate::game::habitat::catalog_prop_by_str(crate::game::habitat::HEAVY_SESSION_PLANTER)
                .unwrap();
        let base = [spec.color.0, spec.color.1, spec.color.2];
        assert!(content
            .glyphs
            .into_iter()
            .zip(paints)
            .any(
                |(glyph, paint)| glyph.glyph.is_some_and(|glyph| glyph.as_char() == '*')
                    && paint.is_some_and(|paint| paint.color_srgb8 == [0xe8, 0x84, 0xbc])
            ));
        assert!(content
            .glyphs
            .into_iter()
            .zip(paints)
            .any(
                |(glyph, paint)| glyph.glyph.is_some_and(|glyph| glyph.as_char() != '*')
                    && paint.is_some_and(|paint| paint.color_srgb8 == base)
            ));
    }

    #[test]
    fn bloom_paint_is_catalog_capability_bound() {
        let bloom_ids = [
            crate::game::habitat::TOKEN_MOSS_TUFT_250K,
            crate::game::habitat::TOKEN_HANGING_VINE_25M,
            crate::game::habitat::HEAVY_SESSION_PLANTER,
            crate::game::habitat::TOKEN_REEDS_5M,
        ];
        for catalog_id in bloom_ids {
            for bloom_active in [false, true] {
                let mut snapshot = snapshot_for(Species::Fuzz, Stage::S3);
                snapshot.topology.visible_props = vec![super::super::PropTopologySnapshot {
                    catalog_id,
                    stable_order: 0,
                    zone: super::super::PropZoneSnapshot::FloorMid,
                    authored_depth: super::super::AuthoredDepthSnapshot::BehindPet,
                }];
                snapshot.content.prop_animation_states =
                    vec![super::super::PropAnimationSnapshot {
                        catalog_id,
                        stable_order: 0,
                        kind: super::super::PropAnimationKindSnapshot::Animated,
                        sprite_phase: Some(0),
                        twinkle_active: None,
                        motion_phase: None,
                        chest_lid_open: None,
                        bloom_active: Some(bloom_active),
                        origin_points: [180.0, 280.0],
                    }];
                let built = build_scene_generation(&snapshot, generation_key(45)).unwrap();
                let content = built.content().prop_slots[0].content.unwrap();
                let paints = built.content().prop_paint_slots[0].paints;
                let spec = crate::game::habitat::catalog_prop_by_str(catalog_id).unwrap();
                let base = [spec.color.0, spec.color.1, spec.color.2];
                let mut saw_blossom = false;
                for (glyph, paint) in content.glyphs.into_iter().zip(paints) {
                    let Some(glyph) = glyph.glyph else {
                        assert!(paint.is_none());
                        continue;
                    };
                    let expected = if bloom_active && glyph.as_char() == '*' {
                        saw_blossom = true;
                        [0xe8, 0x84, 0xbc]
                    } else {
                        base
                    };
                    assert_eq!(paint.unwrap().color_srgb8, expected, "{catalog_id}");
                }
                assert_eq!(saw_blossom, bloom_active, "{catalog_id}");
            }
        }

        for catalog_id in [
            crate::game::habitat::TOKEN_BONSAI_100M,
            crate::game::habitat::TOKEN_CONSTELLATION_250M,
        ] {
            let mut snapshot = snapshot_for(Species::Fuzz, Stage::S3);
            snapshot.topology.visible_props = vec![super::super::PropTopologySnapshot {
                catalog_id,
                stable_order: 0,
                zone: super::super::PropZoneSnapshot::FloorMid,
                authored_depth: super::super::AuthoredDepthSnapshot::BehindPet,
            }];
            snapshot.content.prop_animation_states = vec![super::super::PropAnimationSnapshot {
                catalog_id,
                stable_order: 0,
                kind: super::super::PropAnimationKindSnapshot::Animated,
                sprite_phase: Some(0),
                twinkle_active: None,
                motion_phase: None,
                chest_lid_open: None,
                bloom_active: Some(true),
                origin_points: [180.0, 280.0],
            }];
            assert_eq!(
                build_scene_generation(&snapshot, generation_key(46)),
                Err(SceneGenerationError::SnapshotRejected(
                    super::super::runtime::SnapshotRejection::InconsistentIdentity,
                )),
                "{catalog_id}"
            );
        }
    }

    #[test]
    fn analytic_and_glyph_paint_fields_hash_in_their_owned_domains() {
        let fixture = SceneFixture::valid();
        let content_checksum = checksum::checksum_content(&fixture.content).unwrap();
        let frame_checksum = checksum::checksum_frame(&fixture.template, &fixture.frame).unwrap();

        let mut changed_content = fixture.content.clone();
        changed_content.analytic_slots[4]
            .value
            .as_mut()
            .unwrap()
            .paint = AnalyticPaint::MoodAuraRings {
            color_srgb8: [1, 2, 3],
            ring_count: 8,
            per_ring_alpha_u8: 13,
        };
        assert_ne!(
            checksum::checksum_content(&changed_content).unwrap(),
            content_checksum
        );
        assert_eq!(
            checksum::checksum_frame(&fixture.template, &fixture.frame).unwrap(),
            frame_checksum
        );

        let mut changed_frame = fixture.frame.clone();
        changed_frame.analytic_slots[2]
            .value
            .as_mut()
            .unwrap()
            .rect_points[0] += 1.0;
        assert_ne!(
            checksum::checksum_frame(&fixture.template, &changed_frame).unwrap(),
            frame_checksum
        );
        assert_eq!(
            checksum::checksum_content(&fixture.content).unwrap(),
            content_checksum
        );
    }

    #[test]
    fn exact_activity_gauges_and_dim_remain_same_generation_frame_deltas() {
        let initial = Arc::new(snapshot_with_private_frame(0.1, 0.0));
        let mut built = build_scene_generation_owned(
            Arc::clone(&initial),
            generation_key(43),
            super::super::AppliedRevisions::new(0, 0),
        )
        .unwrap();
        let template_checksum = built.template().generation_checksum;
        let content_checksum = built.content_checksum();

        let mut changed = (*initial).clone();
        changed.frame.activity_recent = true;
        changed.frame.activity_opacity = 0.333_333_34;
        changed.frame.gauge_fractions = [0.125, 0.25, 0.5, 0.875];
        changed.frame.gauge_levels = [
            super::super::GaugeLevelSnapshot::Low,
            super::super::GaugeLevelSnapshot::Medium,
            super::super::GaugeLevelSnapshot::High,
            super::super::GaugeLevelSnapshot::High,
        ];
        changed.frame.dimmed = true;
        changed.frame.dim_amount = 0.456_789;
        let changed = Arc::new(changed);
        let changes = super::super::runtime::classify_snapshot_changes(&initial, &changed);
        assert!(!changes.requires_generation());
        let status = built
            .template()
            .nodes
            .iter()
            .find(|node| node.alias.as_str() == "chrome.status")
            .unwrap()
            .id;
        let delta = built
            .project_snapshot_changes(
                &changed,
                changes,
                super::super::AppliedRevisions::new(0, 0),
                super::super::AppliedRevisions::new(0, 1),
            )
            .unwrap();
        assert!(delta.content.analytic_slots.is_empty());
        assert_eq!(delta.frame.gauges, Some([0.125, 0.25, 0.5, 0.875]));
        assert_eq!(delta.frame.dim_amount, Some(0.456_789));
        assert_eq!(
            delta
                .frame
                .nodes
                .iter()
                .find(|node| node.node == status)
                .unwrap()
                .opacity,
            0.333_333_34
        );

        built
            .apply_compatible_snapshot(
                changed,
                changes,
                super::super::AppliedRevisions::new(0, 0),
                super::super::AppliedRevisions::new(0, 1),
            )
            .unwrap();
        assert_eq!(built.template().generation_checksum, template_checksum);
        assert_eq!(built.content_checksum(), content_checksum);
        assert_eq!(built.frame().gauges, [0.125, 0.25, 0.5, 0.875]);
        assert_eq!(built.frame().dim_amount, 0.456_789);
    }

    #[test]
    fn prop_attachment_composes_instance_motion_inside_rotated_scaled_parent() {
        let mut snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        snapshot.topology.visible_props = vec![super::super::PropTopologySnapshot {
            catalog_id: crate::game::habitat::TOKEN_TREASURE_CHEST_2M,
            stable_order: 0,
            zone: super::super::PropZoneSnapshot::FloorMid,
            authored_depth: super::super::AuthoredDepthSnapshot::BehindPet,
        }];
        snapshot.content.prop_animation_states = vec![super::super::PropAnimationSnapshot {
            catalog_id: crate::game::habitat::TOKEN_TREASURE_CHEST_2M,
            stable_order: 0,
            kind: super::super::PropAnimationKindSnapshot::Animated,
            sprite_phase: None,
            twinkle_active: None,
            motion_phase: None,
            chest_lid_open: Some(true),
            bloom_active: None,
            origin_points: [10.0, 20.0],
        }];
        let mut built = build_scene_generation(&snapshot, generation_key(1)).unwrap();
        let attachment = built.template.attachments[0].clone();
        let source = built
            .template
            .primitives
            .iter()
            .find(|primitive| {
                primitive.binding
                    == PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(0))
            })
            .unwrap()
            .node;

        let mut current = Some(attachment.owner);
        while let Some(id) = current {
            let node = built
                .template
                .nodes
                .iter_mut()
                .find(|node| node.id == id)
                .unwrap();
            current = node.parent;
            node.base_transform = Transform3::IDENTITY;
            built
                .frame
                .nodes
                .iter_mut()
                .find(|node| node.node == id)
                .unwrap()
                .local_transform = Transform3::IDENTITY;
        }
        built
            .template
            .nodes
            .iter_mut()
            .find(|node| node.id == source)
            .unwrap()
            .base_transform = Transform3 {
            rotation_xyzw: [0.0, 0.0, 0.707_106_77, 0.707_106_77],
            scale: [2.0, 3.0, 1.0],
            ..Transform3::IDENTITY
        };
        built
            .frame
            .nodes
            .iter_mut()
            .find(|node| node.node == attachment.owner)
            .unwrap()
            .local_transform = Transform3 {
            translation: [3.0, 4.0, 0.0],
            scale: [0.5, 2.0, 1.0],
            ..Transform3::IDENTITY
        };
        built.template.attachments[0].local = Transform3::translated([1.0, 2.0, 0.0]);
        built.frame.prop_slots[0].origin_points = [10.0, 20.0];
        built.frame.prop_slots[0].motion_offset_points = [0.0; 2];

        let attachment = &built.template.attachments[0];
        let visible_prop_origin = resolve_attachment_world(
            &built.template,
            &built.frame,
            &AttachmentTemplate {
                local: Transform3::IDENTITY,
                owner: source,
                ..attachment.clone()
            },
        )
        .unwrap()
        .transform_point3([0.0; 3]);
        let attached = resolve_attachment_world(&built.template, &built.frame, attachment)
            .unwrap()
            .transform_point3([0.0; 3]);
        assert_point_close(visible_prop_origin, [-60.0, 20.0, 0.0, 1.0]);
        assert_point_close(attached, [-84.0, 27.0, 0.0, 1.0]);

        built.frame.prop_slots[0].motion_offset_points = [1.0, -2.0];
        let moved_visible_prop_origin = resolve_attachment_world(
            &built.template,
            &built.frame,
            &AttachmentTemplate {
                local: Transform3::IDENTITY,
                owner: source,
                ..attachment.clone()
            },
        )
        .unwrap()
        .transform_point3([0.0; 3]);
        let moved_attachment = resolve_attachment_world(&built.template, &built.frame, attachment)
            .unwrap()
            .transform_point3([0.0; 3]);
        assert_point_close(moved_visible_prop_origin, [-54.0, 22.0, 0.0, 1.0]);
        assert_point_close(moved_attachment, [-78.0, 29.0, 0.0, 1.0]);
        assert_point_close(
            [
                moved_attachment[0] - attached[0],
                moved_attachment[1] - attached[1],
                moved_attachment[2] - attached[2],
                1.0,
            ],
            [6.0, 2.0, 0.0, 1.0],
        );
    }

    #[test]
    fn every_tank_variant_and_morph_fits_two_by_eight_point_space_slots() {
        for spec in crate::game::habitat::TANK_INHABITANT_CATALOG {
            for variant in [0, 1] {
                for morph in [None, Some(0), Some(1), Some(2), Some(3)] {
                    let cells =
                        crate::presentation::tank_life::tank_sprite_cells(spec.id, variant, morph);
                    assert!(cells.len() <= MAX_TANK_GLYPHS_PER_SLOT, "{}", spec.id);
                    for cell in cells {
                        AuthoredGlyph::new(cell.glyph).unwrap();
                    }
                }
            }
        }
    }

    #[test]
    fn hierarchy_depth_gauges_and_raw_clock_contracts_are_exact() {
        let mut snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        snapshot.frame.gauge_levels = [
            super::super::GaugeLevelSnapshot::Empty,
            super::super::GaugeLevelSnapshot::Low,
            super::super::GaugeLevelSnapshot::Medium,
            super::super::GaugeLevelSnapshot::High,
        ];
        snapshot.frame.gauge_fractions = [0.0, 0.125, 0.375, 0.75];
        let built = build_scene_generation(&snapshot, generation_key(1)).unwrap();
        let nodes = built
            .template
            .nodes
            .iter()
            .map(|node| (node.alias.as_str(), node))
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            nodes["world.room.background"].base_transform.translation[2],
            -1.90
        );
        assert_eq!(
            nodes["pet.shadow.wall"].base_transform.translation[2],
            -1.30
        );
        assert_eq!(
            nodes["pet.projection.floor"].base_transform.translation[2],
            -1.70
        );
        assert_eq!(built.frame.gauges, [0.0, 0.125, 0.375, 0.75]);
        let mut orders = built
            .template
            .primitives
            .iter()
            .map(|primitive| primitive.authored_order)
            .collect::<Vec<_>>();
        let count = orders.len();
        orders.sort_unstable();
        orders.dedup();
        assert_eq!(orders.len(), count);

        let mut clock_only = snapshot.clone();
        clock_only.frame.elapsed_ms += 1_000;
        let clock_built = build_scene_generation(&clock_only, generation_key(1)).unwrap();
        assert_eq!(
            built.template.generation_checksum,
            clock_built.template.generation_checksum
        );
        assert_eq!(built.content_checksum, clock_built.content_checksum);
        assert_eq!(built.frame_checksum, clock_built.frame_checksum);

        let mut facing_only = snapshot.clone();
        facing_only.frame.facing *= -1;
        let facing_built = build_scene_generation(&facing_only, generation_key(1)).unwrap();
        assert_eq!(
            built.template.generation_checksum,
            facing_built.template.generation_checksum
        );
        assert_eq!(built.content_checksum, facing_built.content_checksum);
        assert_ne!(built.frame_checksum, facing_built.frame_checksum);
    }

    #[test]
    fn exact_depth_and_gauges_match_fresh_and_compatible_projection() {
        let initial = std::sync::Arc::new(snapshot_for(Species::Fuzz, Stage::S3));
        for raw_depth in [-1.0, 0.0, 1.0] {
            let mut desired = (*initial).clone();
            let resolved = crate::round::depth::resolve_smooth_depth(
                raw_depth,
                crate::round::depth::depth_lifecycle_scale(false, false),
            )
            .unwrap();
            desired.content.day_phase = super::super::CompanionDayPhase::Dusk;
            desired.frame.pet_depth = raw_depth;
            desired.frame.pet_depth_cue = super::super::DepthCue {
                scale: resolved.scale,
                y_offset_points_up: -resolved.perspective_y
                    * desired.topology.glyph_grid.cell_extent_points[1],
                opacity: resolved.atmosphere,
                saturation: 1.0,
            };
            desired.frame.calm = false;
            desired.frame.dimmed = false;
            desired.frame.dim_amount = 0.0;
            desired.frame.gauge_fractions = [0.123_456_79, 0.432_109, 0.765_432_1, 1.0];
            desired.frame.gauge_levels = desired
                .frame
                .gauge_fractions
                .map(|value| super::super::GaugeLevelSnapshot::from_fraction(f64::from(value)));
            let desired = std::sync::Arc::new(desired);
            let fresh = build_scene_generation_owned(
                std::sync::Arc::clone(&desired),
                generation_key(1),
                super::super::AppliedRevisions::new(1, 1),
            )
            .unwrap();

            let mut from_initial = build_scene_generation_owned(
                std::sync::Arc::clone(&initial),
                generation_key(1),
                super::super::AppliedRevisions::new(0, 0),
            )
            .unwrap();
            let changes = super::super::runtime::classify_snapshot_changes(&initial, &desired);
            from_initial
                .apply_compatible_snapshot(
                    std::sync::Arc::clone(&desired),
                    changes,
                    super::super::AppliedRevisions::new(0, 0),
                    super::super::AppliedRevisions::new(1, 1),
                )
                .unwrap();

            assert_eq!(from_initial.frame, fresh.frame);
            assert_eq!(from_initial.content, fresh.content);
            assert_eq!(from_initial.frame_checksum, fresh.frame_checksum);
            assert_eq!(from_initial.content_checksum, fresh.content_checksum);
            assert_eq!(
                fresh.content.day_phase,
                super::super::CompanionDayPhase::Dusk
            );
            assert_eq!(fresh.frame.gauges, desired.frame.gauge_fractions);
            let pet = fresh
                .template
                .nodes
                .iter()
                .find(|node| node.alias.as_str() == "pet")
                .unwrap();
            let pet_frame = fresh
                .frame
                .nodes
                .iter()
                .find(|node| node.node == pet.id)
                .unwrap();
            assert_eq!(pet_frame.local_transform.translation[2], raw_depth);
            assert_eq!(pet_frame.local_transform.scale[1], resolved.scale);
            let body = fresh
                .template
                .nodes
                .iter()
                .find(|node| node.alias.as_str() == "pet.body")
                .unwrap();
            let body_frame = fresh
                .frame
                .nodes
                .iter()
                .find(|node| node.node == body.id)
                .unwrap();
            assert_eq!(body_frame.opacity, resolved.atmosphere);
        }
    }

    #[test]
    fn pet_depth_projection_covers_active_calm_and_asleep_lifecycles() {
        for (lifecycle, asleep, calm) in [
            ("active", false, false),
            ("calm", false, true),
            ("asleep", true, true),
        ] {
            for raw_depth in [-1.0, 0.0, 1.0] {
                let mut snapshot = snapshot_for(Species::Fuzz, Stage::S3);
                snapshot.frame.pet_depth = raw_depth;
                set_depth_lifecycle(&mut snapshot, asleep, calm);
                let resolved = crate::round::depth::resolve_smooth_depth(
                    raw_depth,
                    crate::round::depth::depth_lifecycle_scale(asleep, calm),
                )
                .unwrap();
                let generation = build_scene_generation(&snapshot, generation_key(1)).unwrap();
                let node_frame = |alias: &str| {
                    let id = generation
                        .template
                        .nodes
                        .iter()
                        .find(|node| node.alias.as_str() == alias)
                        .unwrap()
                        .id;
                    generation
                        .frame
                        .nodes
                        .iter()
                        .find(|node| node.node == id)
                        .copied()
                        .unwrap()
                };
                let pet = node_frame("pet");
                let body = node_frame("pet.body");
                let particles = node_frame("pet.particles");
                let wall = node_frame("pet.shadow.wall");
                let floor = node_frame("pet.projection.floor");
                let cell = snapshot.topology.glyph_grid.cell_extent_points;
                let extent = [
                    f32::from(super::super::PET_LATTICE_WIDTH) * cell[0],
                    f32::from(super::super::PET_LATTICE_HEIGHT) * cell[1],
                ];
                let expected_y = snapshot.topology.layout.height_points
                    - snapshot.frame.pet_anchor_points[1]
                    - snapshot.frame.breath_offset_y_points
                    - snapshot.frame.bob_offset_y_points
                    - extent[1]
                    - resolved.perspective_y * cell[1];

                assert_eq!(
                    pet.local_transform.translation,
                    [snapshot.frame.pet_anchor_points[0], expected_y, raw_depth],
                    "{lifecycle} at depth {raw_depth}"
                );
                assert_eq!(
                    pet.local_transform.scale,
                    [resolved.scale, resolved.scale, 1.0],
                    "{lifecycle} at depth {raw_depth}"
                );
                assert_eq!(
                    pet.local_transform.pivot,
                    [extent[0] * 0.5, extent[1] * 0.5, 0.0],
                    "{lifecycle} at depth {raw_depth}"
                );
                assert_eq!(
                    body.opacity,
                    if asleep { 0.65 } else { 1.0 } * resolved.atmosphere,
                    "{lifecycle} at depth {raw_depth}"
                );
                assert_eq!(
                    particles.visible, !asleep,
                    "{lifecycle} at depth {raw_depth}"
                );
                assert_eq!(
                    particles.opacity, resolved.atmosphere,
                    "{lifecycle} at depth {raw_depth}"
                );
                let wall_cue = crate::presentation::companion_effects::wall_shadow_depth_cue(
                    resolved.effective_z,
                );
                assert_eq!(wall.opacity, wall_cue.strength);
                assert!(
                    generation.frame.analytic_slots[1].value.unwrap().geometry
                        == AnalyticGeometry::PetSilhouette {
                            mask: AnalyticMaskSource::PetBody,
                            offset_points: [
                                wall_cue.detach_cells * cell[0],
                                -wall_cue.detach_cells * cell[1],
                            ],
                            softness_points: (cell[0].min(cell[1]) * 0.35).max(1.0),
                        }
                );

                let pet_center = [
                    pet.local_transform.translation[0] + pet.local_transform.pivot[0],
                    pet.local_transform.translation[1] + pet.local_transform.pivot[1],
                ];
                let projection = crate::presentation::companion_effects::floor_projection_metrics(
                    snapshot.topology.layout.width_points,
                    snapshot.topology.layout.height_points,
                    snapshot.topology.layout.height_points * 0.76,
                    snapshot.topology.layout.height_points,
                    pet_center[0],
                    resolved.effective_z,
                )
                .unwrap();
                assert_eq!(floor.opacity, f32::from(projection.alpha) / 235.0);
                assert!(
                    generation.frame.analytic_slots[2].value.unwrap().geometry
                        == AnalyticGeometry::RadialEllipse {
                            center_points: [
                                projection.center_x,
                                snapshot.topology.layout.height_points - projection.center_y,
                            ],
                            radii_points: [projection.radius_x, projection.radius_y],
                            softness_points: projection.radius_y,
                        }
                );

                let (body_center, body_radii) =
                    super::compiler::pet_body_world_geometry(&snapshot, pet.local_transform)
                        .unwrap();
                assert!(body_radii[0] < extent[0] * resolved.scale * 0.5);
                let aura_radius = crate::presentation::companion_effects::mood_aura_radius(
                    f64::from(body_radii[0] * 2.0),
                ) as f32;
                assert!(
                    generation.frame.analytic_slots[4].value.unwrap().geometry
                        == AnalyticGeometry::PetAura {
                            center_points: body_center,
                            max_radius_points: aura_radius,
                            ring_count: 8,
                            feather_points: 4.0,
                        }
                );

                let aura_node = generation
                    .template
                    .nodes
                    .iter()
                    .find(|node| node.alias.as_str() == "pet.aura.mood")
                    .unwrap();
                let root = generation
                    .template
                    .nodes
                    .iter()
                    .find(|node| node.alias.as_str() == "scene.root")
                    .unwrap();
                assert_eq!(aura_node.parent, Some(root.id));
                let status_tone = match generation.frame.analytic_slots[3].value.unwrap().geometry {
                    AnalyticGeometry::StatusBeacon { tone, .. } => tone,
                    _ => unreachable!(),
                };
                assert_eq!(
                    status_tone,
                    if calm || asleep {
                        StatusBeaconTone::Calm
                    } else {
                        StatusBeaconTone::Active
                    }
                );
            }
        }
    }

    #[test]
    fn floor_projection_ignores_pet_breath_and_bob() {
        let baseline = snapshot_for(Species::Fuzz, Stage::S3);
        let mut moved = baseline.clone();
        moved.frame.breath_offset_y_points += 7.25;
        moved.frame.bob_offset_y_points -= 3.5;

        let baseline = build_scene_generation(&baseline, generation_key(1)).unwrap();
        let moved = build_scene_generation(&moved, generation_key(1)).unwrap();
        assert!(baseline.frame.analytic_slots[2] == moved.frame.analytic_slots[2]);
        assert!(baseline.frame.analytic_slots[4] != moved.frame.analytic_slots[4]);
    }

    #[test]
    fn retained_pet_transform_ignores_stepped_breath_offset() {
        let baseline = snapshot_for(Species::Fuzz, Stage::S3);
        let mut breathing = baseline.clone();
        breathing.frame.breath_offset_y_points = baseline.topology.glyph_grid.cell_extent_points[1];

        let pet_transform = |snapshot: &CompanionSceneSnapshot| {
            let generation = build_scene_generation(snapshot, generation_key(1)).unwrap();
            let pet_id = generation
                .template
                .nodes
                .iter()
                .find(|node| node.alias.as_str() == "pet")
                .unwrap()
                .id;
            generation
                .frame
                .nodes
                .iter()
                .find(|node| node.node == pet_id)
                .unwrap()
                .local_transform
        };

        assert_eq!(pet_transform(&baseline), pet_transform(&breathing));
    }

    #[test]
    fn aura_uses_tight_asymmetric_body_bounds_not_particle_lattice_bounds() {
        let mut snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        snapshot.content.pet_lines =
            vec!["             ".to_owned(); usize::from(super::super::PET_LATTICE_HEIGHT)];
        snapshot.content.pet_lines[3].replace_range(2..3, "^");
        snapshot.content.pet_roles = vec![super::super::PetRoleSpanSnapshot {
            line_index: 3,
            start_char: 2,
            end_char: 3,
            role: "body",
        }];
        let built = build_scene_generation(&snapshot, generation_key(49)).unwrap();
        let pet_node = built
            .template
            .nodes
            .iter()
            .find(|node| node.alias.as_str() == "pet")
            .unwrap();
        let transform = built
            .frame
            .nodes
            .iter()
            .find(|node| node.node == pet_node.id)
            .unwrap()
            .local_transform;
        let cell = snapshot.topology.glyph_grid.cell_extent_points;
        let local_center = [
            2.5 * cell[0],
            (f32::from(super::super::PET_LATTICE_HEIGHT) - 3.5) * cell[1],
            0.0,
        ];
        let expected = transform.matrix().unwrap().transform_point3(local_center);
        let expected_radius = crate::presentation::companion_effects::mood_aura_radius(f64::from(
            cell[0] * transform.scale[0].abs(),
        )) as f32;
        assert!(
            built.frame.analytic_slots[4].value.unwrap().geometry
                == AnalyticGeometry::PetAura {
                    center_points: [expected[0], expected[1]],
                    max_radius_points: expected_radius,
                    ring_count: 8,
                    feather_points: 4.0,
                }
        );
    }

    #[test]
    fn overlapping_pet_roles_fail_instead_of_last_writer_wins() {
        let mut snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        snapshot.content.pet_roles = vec![
            super::super::PetRoleSpanSnapshot {
                line_index: 0,
                start_char: 0,
                end_char: 2,
                role: "body",
            },
            super::super::PetRoleSpanSnapshot {
                line_index: 0,
                start_char: 1,
                end_char: 3,
                role: "eye",
            },
        ];
        assert_eq!(
            build_scene_generation(&snapshot, generation_key(1)),
            Err(SceneGenerationError::OverlappingPetRole)
        );
    }

    #[test]
    fn full_ten_prop_two_tank_state_matrix_has_no_topology_churn() {
        let mut base = snapshot_for(Species::Fuzz, Stage::S6);
        base.topology.visible_props = crate::game::habitat::HABITAT_PROP_CATALOG
            .iter()
            .take(MAX_VISIBLE_PROPS)
            .enumerate()
            .map(|(index, spec)| super::super::PropTopologySnapshot {
                catalog_id: spec.id,
                stable_order: index as u8,
                zone: spec.zone.into(),
                authored_depth: spec.pet_layer.into(),
            })
            .collect();
        base.content.prop_animation_states = base
            .topology
            .visible_props
            .iter()
            .map(|prop| super::super::PropAnimationSnapshot {
                catalog_id: prop.catalog_id,
                stable_order: prop.stable_order,
                kind: super::super::PropAnimationKindSnapshot::Animated,
                sprite_phase: Some(0),
                twinkle_active: Some(false),
                motion_phase: Some(0),
                chest_lid_open: (prop.catalog_id == crate::game::habitat::TOKEN_TREASURE_CHEST_2M)
                    .then_some(false),
                bloom_active: crate::game::habitat::habitat_prop_supports_bloom(prop.catalog_id)
                    .then_some(false),
                origin_points: [20.0 + f32::from(prop.stable_order) * 24.0, 280.0],
            })
            .collect();
        base.topology.visible_tank_inhabitants = crate::game::habitat::TANK_INHABITANT_CATALOG
            .iter()
            .take(2)
            .enumerate()
            .map(|(index, spec)| super::super::TankTopologySnapshot {
                catalog_id: spec.id,
                stable_order: index as u8,
                route: spec.route_family.into(),
                authored_depth: spec.natural_layer.into(),
            })
            .collect();
        base.content.tank_animation_states = base
            .topology
            .visible_tank_inhabitants
            .iter()
            .map(|tank| super::super::TankAnimationSnapshot {
                catalog_id: tank.catalog_id,
                stable_order: tank.stable_order,
                route: tank.route,
                visible: true,
                origin_col: 1,
                origin_row: 1,
                origin_points: [40.0 + f32::from(tank.stable_order) * 40.0, 120.0],
                side: None,
                layer: super::super::TankLayerSnapshot::Behind,
                sprite_variant: 0,
                visible_rows: 1,
                anemone_morph: None,
                color_srgb8: crate::presentation::tank_life::tank_paint_for(tank.catalog_id)
                    .expect("catalog fixture paint")
                    .color_srgb8,
                bold: crate::presentation::tank_life::tank_paint_for(tank.catalog_id)
                    .expect("catalog fixture paint")
                    .bold,
                cadence_ms: 4_000,
                calm: false,
                cells: vec![super::super::TankCellSnapshot {
                    col: 1,
                    row: 1,
                    glyph: if tank.stable_order == 0 { '╭' } else { '‹' },
                    layer: super::super::TankLayerSnapshot::Behind,
                    position_points: [40.0, 120.0],
                }],
                bounds: Some(super::super::TankBoundsSnapshot { x: 1, y: 1, width: 1, height: 1 }),
                bounds_points: Some([40.0, 120.0, 8.0, 12.0]),
            })
            .collect();

        let normal = build_scene_generation(&base, generation_key(1)).unwrap();
        let content_debug = format!("{:?}", normal.content());
        assert!(content_debug.contains("color_srgb8"));
        assert!(content_debug.contains("bold: true"));
        let mut changed_paint = normal.content().clone();
        changed_paint.tank_slots[0]
            .content
            .as_mut()
            .expect("occupied tank content")
            .color_srgb8[0] ^= 1;
        assert_ne!(
            super::checksum::checksum_content(&changed_paint).unwrap(),
            normal.content_checksum()
        );
        let mut changed_weight = normal.content().clone();
        changed_weight.tank_slots[0]
            .content
            .as_mut()
            .expect("occupied tank content")
            .bold = false;
        assert_ne!(
            super::checksum::checksum_content(&changed_weight).unwrap(),
            normal.content_checksum()
        );
        for (asleep, helper, dim) in [
            (false, false, 0.0),
            (false, true, 0.0),
            (true, false, 0.35),
            (true, true, 0.35),
        ] {
            let mut state = base.clone();
            set_depth_lifecycle(&mut state, asleep, asleep);
            state.frame.helper_trouble = helper;
            state.frame.dimmed = dim > 0.0;
            state.frame.dim_amount = dim;
            let built = build_scene_generation(&state, generation_key(1)).unwrap();
            assert_eq!(
                built.template.generation_checksum,
                normal.template.generation_checksum
            );
            assert_eq!(
                built
                    .content
                    .prop_slots
                    .iter()
                    .filter(|slot| slot.content.is_some())
                    .count(),
                10
            );
            assert_eq!(
                built
                    .content
                    .tank_slots
                    .iter()
                    .filter(|slot| slot.content.is_some())
                    .count(),
                2
            );
            assert!(built.frame.lights.is_empty());
        }
    }

    #[test]
    fn canonical_aliases_produce_stable_fnv1a_ids() {
        let alias = CanonicalAlias::new("world.prop.treasure_chest").unwrap();
        assert_eq!(NodeId::from_alias(&alias), NodeId(0xa64f_d17f));
        assert!(CanonicalAlias::new("World.Prop").is_err());
        assert!(CanonicalAlias::new("world//prop").is_err());
        assert!(CanonicalAlias::new("").is_err());
    }

    #[test]
    fn y_down_projection_becomes_y_up_once() {
        let transform = Transform3::from_snapshot_xy_depth([10.0, 20.0, 0.5], 100.0);
        assert_eq!(transform.translation, [10.0, 80.0, 0.5]);
    }

    #[test]
    fn identity_transform_uses_column_major_column_vectors() {
        let matrix = Transform3::IDENTITY.matrix().unwrap();
        assert_eq!(matrix, Mat4::IDENTITY);
        assert_eq!(
            matrix.transform_point3([2.0, 3.0, 4.0]),
            [2.0, 3.0, 4.0, 1.0]
        );
    }

    #[test]
    fn pivot_stays_fixed_while_scale_moves_surrounding_points() {
        let transform = Transform3 {
            translation: [3.0, -4.0, 0.0],
            rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale: [2.0, 2.0, 2.0],
            pivot: [10.0, 20.0, 0.0],
        };
        let matrix = transform.matrix().unwrap();
        assert_point_close(
            matrix.transform_point3([10.0, 20.0, 0.0]),
            [13.0, 16.0, 0.0, 1.0],
        );
        assert_point_close(
            matrix.transform_point3([11.0, 20.0, 0.0]),
            [15.0, 16.0, 0.0, 1.0],
        );
    }

    #[test]
    fn parent_world_matrix_multiplies_child_local_matrix() {
        let parent = Transform3::translated([5.0, 7.0, 0.0]).matrix().unwrap();
        let child = Transform3::translated([2.0, 3.0, 0.0]).matrix().unwrap();
        assert_point_close(
            (parent * child).transform_point3([0.0, 0.0, 0.0]),
            [7.0, 10.0, 0.0, 1.0],
        );
    }

    #[test]
    fn right_handed_active_quaternion_rotates_x_toward_y() {
        let half = std::f32::consts::FRAC_PI_4;
        let transform = Transform3 {
            rotation_xyzw: [0.0, 0.0, half.sin(), half.cos()],
            ..Transform3::IDENTITY
        };
        assert_point_close(
            transform
                .matrix()
                .unwrap()
                .transform_point3([1.0, 0.0, 0.0]),
            [0.0, 1.0, 0.0, 1.0],
        );
    }

    #[test]
    fn huge_finite_quaternion_normalizes_without_overflow() {
        let transform = Transform3 {
            rotation_xyzw: [f32::MAX, f32::MAX, f32::MAX, f32::MAX],
            ..Transform3::IDENTITY
        };
        let matrix = transform.matrix().unwrap();
        assert!(matrix
            .columns
            .iter()
            .flatten()
            .all(|value| value.is_finite()));
    }

    #[test]
    fn orthographic_depth_maps_near_to_zero_and_far_to_one() {
        let camera = OrthographicCamera::new(360.0, 360.0, -2.0, 2.0).unwrap();
        assert_eq!(camera.clip_depth(2.0), Ok(0.0));
        assert_eq!(camera.clip_depth(-2.0), Ok(1.0));
        assert_eq!(camera.clip_depth(0.0), Ok(0.5));
        let matrix = camera.projection_matrix().unwrap();
        assert_point_close(
            matrix.transform_point3([0.0, 0.0, 2.0]),
            [-1.0, -1.0, 0.0, 1.0],
        );
        assert_point_close(
            matrix.transform_point3([360.0, 360.0, -2.0]),
            [1.0, 1.0, 1.0, 1.0],
        );
    }

    #[test]
    fn transform_and_camera_reject_non_finite_or_degenerate_inputs() {
        assert_eq!(
            Transform3 {
                translation: [f32::NAN, 0.0, 0.0],
                ..Transform3::IDENTITY
            }
            .matrix(),
            Err(TransformError::NonFinite)
        );
        assert_eq!(
            Transform3 {
                rotation_xyzw: [0.0; 4],
                ..Transform3::IDENTITY
            }
            .matrix(),
            Err(TransformError::ZeroQuaternion)
        );
        assert_eq!(
            OrthographicCamera::new(360.0, 360.0, 2.0, 2.0),
            Err(CameraError::InvalidDepthRange)
        );
        assert_eq!(
            OrthographicCamera::new(f32::INFINITY, 360.0, -2.0, 2.0),
            Err(CameraError::NonFinite)
        );
        assert_eq!(
            OrthographicCamera::new(f32::from_bits(1), 360.0, -2.0, 2.0),
            Err(CameraError::InvalidExtent)
        );
        assert_eq!(
            OrthographicCamera::new(360.0, 360.0, -f32::MAX, f32::MAX),
            Err(CameraError::InvalidDepthRange)
        );
        let camera = OrthographicCamera::new(360.0, 360.0, -2.0, 2.0).unwrap();
        assert_eq!(
            camera.clip_depth(f32::INFINITY),
            Err(CameraError::NonFinite)
        );
    }

    #[test]
    fn content_values_are_closed_and_pet_glyphs_use_declared_repertoires() {
        use crate::pet::generation::Species;

        assert!(PetGlyph::for_species('^', Species::Fuzz).is_ok());
        assert_eq!(
            PetGlyph::for_species('\n', Species::Fuzz),
            Err(ContentValueError::InvalidPetGlyph)
        );
        assert_eq!(
            PetGlyph::for_species('\u{1f4a5}', Species::Fuzz),
            Err(ContentValueError::InvalidPetGlyph)
        );
        let _roles = [
            PetPaletteRole::Body,
            PetPaletteRole::BodyGlow,
            PetPaletteRole::Eye,
            PetPaletteRole::Mouth,
            PetPaletteRole::Accent,
            PetPaletteRole::Pattern,
            PetPaletteRole::Particle,
            PetPaletteRole::Corruption,
        ];
    }

    #[test]
    fn retained_scene_v2_has_typed_sources_and_fixed_semantic_tables() {
        let snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        let built = build_scene_generation(&snapshot, generation_key(1)).unwrap();

        assert_eq!(MAX_STATIC_ATLAS_RECIPES, 8);
        assert_eq!(MAX_ANALYTIC_PARAMS, 16);
        assert_eq!(
            built.template.static_atlas_recipes.len(),
            MAX_STATIC_ATLAS_RECIPES
        );
        assert!(built
            .template
            .static_atlas_recipes
            .iter()
            .all(|slot| slot.recipe.is_none()));

        let expected = [
            AnalyticSemantic::RoomBackground,
            AnalyticSemantic::WallShadow,
            AnalyticSemantic::FloorProjection,
            AnalyticSemantic::StatusHalo,
            AnalyticSemantic::MoodAura,
            AnalyticSemantic::Gauges,
            AnalyticSemantic::Trouble,
            AnalyticSemantic::Dim,
        ];
        assert_eq!(built.template.analytic_templates.len(), MAX_ANALYTIC_PARAMS);
        for (slot, semantic) in expected.into_iter().enumerate() {
            let id = AnalyticParamId(slot as u8);
            assert_eq!(built.template.analytic_templates[slot].id, id);
            assert_eq!(
                built.template.analytic_templates[slot]
                    .value
                    .as_ref()
                    .map(|value| value.semantic),
                Some(semantic)
            );
        }
        assert!(built.template.analytic_templates[8..]
            .iter()
            .all(|slot| slot.value.is_none()));

        let primitive = |alias: &str| {
            let node = NodeId::from_alias(&CanonicalAlias::new(alias).unwrap());
            built
                .template
                .primitives
                .iter()
                .find(|primitive| primitive.node == node)
                .unwrap()
        };
        assert_eq!(
            primitive("world.room.glyphs").binding,
            PrimitiveBinding::Instances(InstanceGroupBinding::RoomGlyphs)
        );
        assert_eq!(
            primitive("pet.body").binding,
            PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Body))
        );
        assert_eq!(
            primitive("pet.particles").binding,
            PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Particles))
        );
        assert_eq!(
            primitive("world.room.glyphs").blend,
            WorldBlend::PremultipliedAlpha
        );
        assert_eq!(
            primitive("world.room.glyphs").depth,
            DepthBehavior::WorldReadOnly
        );
        assert_eq!(primitive("pet.body").blend, WorldBlend::PremultipliedAlpha);
        assert_eq!(primitive("pet.body").depth, DepthBehavior::WorldReadOnly);
        assert_eq!(
            primitive("pet.projection.floor").blend,
            WorldBlend::Multiply
        );
        let status = primitive("chrome.status");
        assert_eq!(status.blend, WorldBlend::PremultipliedAlpha);
        assert_eq!(status.depth, DepthBehavior::ScreenNoDepth);
        assert_eq!(status.space, PrimitiveSpace::Screen);
        assert_eq!(
            built
                .template
                .materials
                .iter()
                .find(|material| material.id == status.material)
                .unwrap()
                .kind,
            MaterialKind::ScreenChrome
        );
        let status_node = built
            .template
            .nodes
            .iter()
            .find(|node| node.id == status.node)
            .unwrap();
        assert_eq!(
            status_node.parent,
            Some(NodeId::from_alias(
                &CanonicalAlias::new("chrome.screen").unwrap()
            ))
        );

        let mood = primitive("pet.aura.mood");
        assert_eq!(mood.blend, WorldBlend::PremultipliedAlpha);
        assert_eq!(mood.depth, DepthBehavior::WorldReadOnly);
        assert_eq!(mood.space, PrimitiveSpace::World);
        assert_eq!(
            built
                .template
                .materials
                .iter()
                .find(|material| material.id == mood.material)
                .unwrap()
                .kind,
            MaterialKind::UnlitAnalytic
        );

        let order = |alias: &str| primitive(alias).authored_order;
        assert!(order("world.room.glyphs") < order("pet.projection.floor"));
        assert!(order("pet.projection.floor") < order("world.ambient"));
        assert!(order("world.ambient") < order("pet.shadow.wall"));
        assert!(order("pet.shadow.wall") < order("pet.body"));
        let chrome_orders = [
            order("chrome.gauges"),
            order("chrome.status"),
            order("chrome.trouble"),
            order("chrome.hud"),
            order("chrome.dim"),
        ];
        assert!(chrome_orders.windows(2).all(|pair| pair[0] + 1 == pair[1]));

        let wall_order = order("pet.shadow.wall");
        let ambient_order = order("world.ambient");
        for primitive in &built.template.primitives {
            match primitive.binding {
                PrimitiveBinding::Instances(InstanceGroupBinding::PropGlyphs(slot)) => {
                    let depth = snapshot
                        .topology
                        .visible_props
                        .iter()
                        .find(|prop| prop.stable_order == slot)
                        .unwrap()
                        .authored_depth;
                    if depth == super::super::AuthoredDepthSnapshot::Foreground {
                        assert!(primitive.authored_order > order("pet.body"));
                    } else {
                        assert!(primitive.authored_order > ambient_order);
                        assert!(primitive.authored_order < wall_order);
                    }
                }
                PrimitiveBinding::Instances(InstanceGroupBinding::TankCells {
                    layer: InstanceLayer::Behind,
                    ..
                }) => {
                    assert!(primitive.authored_order > ambient_order);
                    assert!(primitive.authored_order < wall_order);
                }
                PrimitiveBinding::Instances(InstanceGroupBinding::TankCells {
                    layer: InstanceLayer::Foreground,
                    ..
                }) => assert!(primitive.authored_order > order("pet.body")),
                _ => {}
            }
        }
        assert!(!built
            .template
            .primitives
            .iter()
            .any(|primitive| { matches!(primitive.binding, PrimitiveBinding::StaticAtlas(_)) }));
    }

    #[test]
    fn production_projection_closes_all_eight_analytic_roles_with_y_up_geometry() {
        let snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        let built = build_scene_generation(&snapshot, generation_key(41)).unwrap();

        assert_eq!(built.content().analytic_slots.len(), MAX_ANALYTIC_PARAMS);
        assert_eq!(built.frame().analytic_slots.len(), MAX_ANALYTIC_PARAMS);
        for (index, semantic) in AnalyticSemantic::ALL.into_iter().enumerate() {
            let content = built.content().analytic_slots[index].value.unwrap();
            let frame = built.frame().analytic_slots[index].value.unwrap();
            assert_eq!(content.semantic, semantic);
            assert_eq!(content.shape, semantic.shape());
            assert_eq!(frame.semantic, semantic);
            assert_eq!(frame.shape, semantic.shape());
            assert!(frame.rect_points.into_iter().all(f32::is_finite));
            assert!(frame.rect_points[2] > 0.0 && frame.rect_points[3] > 0.0);
        }
        assert!(built.content().analytic_slots[8..]
            .iter()
            .all(|slot| slot.value.is_none()));
        assert!(built.frame().analytic_slots[8..]
            .iter()
            .all(|slot| slot.value.is_none()));

        let room = built.frame().analytic_slots[0].value.unwrap();
        assert_eq!(room.rect_points, [0.0, 0.0, 360.0, 360.0]);
        assert!(matches!(
            room.geometry,
            AnalyticGeometry::ApertureRadial {
                center_points: [179.5, 179.5],
                radius_points: 179.0,
                ..
            }
        ));
        assert!(matches!(
            built.frame().analytic_slots[1].value.unwrap().geometry,
            AnalyticGeometry::PetSilhouette { mask: AnalyticMaskSource::PetBody, .. }
        ));
        assert!(matches!(
            built.content().analytic_slots[5].value.unwrap().paint,
            AnalyticPaint::PerimeterGaugeSet { .. }
        ));
    }

    #[test]
    fn analytic_gauges_use_canonical_named_layout_and_paint_on_non_square_views() {
        let mut snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        snapshot.topology.layout.width_points = 420.0;
        snapshot.topology.glyph_grid.cell_extent_points[0] =
            420.0 / f32::from(snapshot.topology.glyph_grid.columns);
        let built = build_scene_generation(&snapshot, generation_key(47)).unwrap();

        let frame = built.frame().analytic_slots[5].value.unwrap();
        let expected = crate::presentation::companion_effects::perimeter_gauge_layout(
            179.0,
            crate::presentation::companion_effects::COMPANION_GAUGE_GAP_DEGREES,
        );
        let actual = match frame.geometry {
            AnalyticGeometry::PerimeterGaugeSet { center_points, xp, daily, pace } => {
                assert_eq!(center_points, [209.5, 179.5]);
                [xp, daily, pace]
            }
            _ => panic!("gauge semantic must carry gauge geometry"),
        };
        for (actual, expected) in
            actual
                .into_iter()
                .zip([expected.xp, expected.daily, expected.pace])
        {
            assert_eq!(actual.radius_points, expected.radius as f32);
            assert_eq!(actual.stroke_width_points, expected.stroke_width as f32);
            assert_eq!(
                actual.track_start_degrees,
                expected.track_start_degrees as f32
            );
            assert_eq!(
                actual.track_sweep_degrees,
                expected.track_sweep_degrees as f32
            );
            assert_eq!(actual.cap, GaugeLineCap::Round);
        }

        let to_srgba8 = crate::presentation::companion_effects::srgba8;
        assert_eq!(
            built.content().analytic_slots[5].value.unwrap().paint,
            AnalyticPaint::PerimeterGaugeSet {
                xp: GaugeLanePaint {
                    track_srgba8: to_srgba8(
                        crate::presentation::companion_effects::GAUGE_XP_TRACK_SRGBA,
                    ),
                    fill_srgba8: to_srgba8(
                        crate::presentation::companion_effects::GAUGE_XP_FILL_SRGBA,
                    ),
                },
                daily: GaugeLanePaint {
                    track_srgba8: to_srgba8(
                        crate::presentation::companion_effects::GAUGE_DAILY_TRACK_SRGBA,
                    ),
                    fill_srgba8: to_srgba8(
                        crate::presentation::companion_effects::GAUGE_DAILY_FILL_SRGBA,
                    ),
                },
                pace: GaugeLanePaint {
                    track_srgba8: to_srgba8(
                        crate::presentation::companion_effects::GAUGE_PACE_TRACK_SRGBA,
                    ),
                    fill_srgba8: to_srgba8(
                        crate::presentation::companion_effects::GAUGE_PACE_FILL_SRGBA,
                    ),
                },
                daily_overage_srgba8: to_srgba8(
                    crate::presentation::companion_effects::GAUGE_DAILY_OVERAGE_SRGBA,
                ),
            }
        );
    }

    #[test]
    fn analytic_room_and_floor_paint_use_shared_biome_phase_authority() {
        for biome in [
            "starter",
            "botanical",
            "technical",
            "celestial",
            "artifact",
            "cozy",
        ] {
            for (phase, scale) in [
                (super::super::CompanionDayPhase::Dawn, 0.85),
                (super::super::CompanionDayPhase::Day, 1.0),
                (super::super::CompanionDayPhase::Dusk, 0.8),
                (super::super::CompanionDayPhase::Night, 0.6),
            ] {
                let mut snapshot = snapshot_for(Species::Fuzz, Stage::S3);
                snapshot.topology.room.primary_biome = biome;
                snapshot.content.day_phase = phase;
                let built = build_scene_generation(&snapshot, generation_key(48)).unwrap();
                let (core_srgb8, rim_srgb8) =
                    crate::presentation::companion_effects::tank_background_paint_srgb8(
                        biome, scale,
                    );
                assert_eq!(
                    built.content().analytic_slots[0].value.unwrap().paint,
                    AnalyticPaint::ApertureDepth { core_srgb8, rim_srgb8 },
                    "{biome} {phase:?}"
                );
                let shadow = crate::presentation::companion_effects::bed_shadow_srgb8(biome);
                assert_eq!(
                    built.content().analytic_slots[2].value.unwrap().paint,
                    AnalyticPaint::FloorShadowMultiplyRadial {
                        inner_srgba8: [shadow[0], shadow[1], shadow[2], 235],
                        outer_srgba8: [shadow[0], shadow[1], shadow[2], 0],
                    },
                    "{biome} {phase:?}"
                );
            }
        }
    }

    #[test]
    fn neutral_template_checksum_covers_fixed_semantic_tables() {
        let checksum = |template: &SceneTemplate| checksum::checksum_template(template).unwrap();
        let baseline = SceneFixture::valid().template;
        let expected = checksum(&baseline);

        let assert_changed = |mutate: fn(&mut SceneTemplate)| {
            let mut changed = baseline.clone();
            mutate(&mut changed);
            assert_ne!(checksum(&changed), expected);
        };

        assert_changed(|template| template.static_atlas_recipes[0].id = StaticAtlasRecipeId(7));
        assert_changed(|template| template.static_atlas_recipes[0].recipe = None);
        assert_changed(|template| {
            template.static_atlas_recipes[0]
                .recipe
                .as_mut()
                .unwrap()
                .source
                .0 ^= 1;
        });
        assert_changed(|template| {
            template.static_atlas_recipes[0]
                .recipe
                .as_mut()
                .unwrap()
                .local_bounds
                .max[0] = 0.5;
        });

        assert_changed(|template| template.analytic_templates[0].id = AnalyticParamId(15));
        assert_changed(|template| template.analytic_templates[0].value = None);
        assert_changed(|template| {
            template.analytic_templates[0]
                .value
                .as_mut()
                .unwrap()
                .semantic = AnalyticSemantic::MoodAura;
        });
        assert_changed(|template| {
            template.analytic_templates[0].value.as_mut().unwrap().shape =
                AnalyticShape::SurfaceOverlay;
        });
        assert_changed(|template| {
            template.analytic_templates[0]
                .value
                .as_mut()
                .unwrap()
                .normalized_local_bounds
                .max[1] = 0.5;
        });
    }

    #[test]
    fn typed_source_validation_rejects_missing_duplicate_and_dangling_bindings() {
        let snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        let template = build_scene_generation(&snapshot, generation_key(1))
            .unwrap()
            .template;

        let mut missing_room = template.clone();
        let room = missing_room
            .primitives
            .iter_mut()
            .find(|primitive| {
                primitive.binding == PrimitiveBinding::Instances(InstanceGroupBinding::RoomGlyphs)
            })
            .unwrap();
        room.binding = PrimitiveBinding::Instances(InstanceGroupBinding::Ambient);
        assert_eq!(
            super::super::validate::validate_template(&missing_room),
            Err(super::super::validate::SceneValidationError::InvalidPrimitiveBinding)
        );

        let mut duplicate_body = template.clone();
        let mut body = duplicate_body
            .primitives
            .iter()
            .find(|primitive| {
                primitive.binding
                    == PrimitiveBinding::Instances(InstanceGroupBinding::PetArt(PetArtFilter::Body))
            })
            .unwrap()
            .clone();
        body.authored_order = duplicate_body.primitives.len() as u16;
        duplicate_body.primitives.push(body);
        assert_eq!(
            super::super::validate::validate_template(&duplicate_body),
            Err(super::super::validate::SceneValidationError::InvalidPrimitiveBinding)
        );

        let mut dangling_analytic = template.clone();
        dangling_analytic
            .primitives
            .iter_mut()
            .find(|primitive| {
                primitive.binding == PrimitiveBinding::Analytic(AnalyticSemantic::StatusHalo.id())
            })
            .unwrap()
            .binding = PrimitiveBinding::Analytic(AnalyticParamId(15));
        assert_eq!(
            super::super::validate::validate_template(&dangling_analytic),
            Err(super::super::validate::SceneValidationError::InvalidPrimitiveBinding)
        );

        let mut nonempty_static_recipe = template.clone();
        let source = CanonicalAlias::new("sprite.future-decoration").unwrap();
        nonempty_static_recipe.static_atlas_recipes[0].recipe = Some(StaticAtlasRecipe {
            semantic: StaticAtlasSemantic::DecorativeSprite,
            source: StaticAtlasSourceKey::from_alias(&source),
            local_bounds: Bounds3 { min: [0.0; 3], max: [1.0, 1.0, 0.0] },
        });
        assert_eq!(
            super::super::validate::validate_template(&nonempty_static_recipe),
            Err(super::super::validate::SceneValidationError::InvalidPrimitiveBinding)
        );

        let mut mismatched_shape = template;
        mismatched_shape.analytic_templates[0]
            .value
            .as_mut()
            .unwrap()
            .shape = AnalyticShape::SurfaceOverlay;
        assert_eq!(
            super::super::validate::validate_template(&mismatched_shape),
            Err(super::super::validate::SceneValidationError::NonCanonicalEmptySlot)
        );
    }

    #[test]
    fn frame_delta_debug_redacts_exact_gauges_and_dim() {
        let mut baseline =
            build_scene_generation(&snapshot_with_private_frame(0.0, 0.0), generation_key(71))
                .unwrap();
        let first = Arc::new(snapshot_with_private_frame(0.312_345_68, 0.567_890_1));
        let first_changes =
            super::super::runtime::classify_snapshot_changes(baseline.source_snapshot(), &first);
        let first_debug = format!(
            "{:?}",
            baseline
                .project_snapshot_changes(
                    &first,
                    first_changes,
                    super::super::AppliedRevisions::new(0, 0),
                    super::super::AppliedRevisions::new(1, 1),
                )
                .unwrap()
                .frame
        );

        let mut baseline =
            build_scene_generation(&snapshot_with_private_frame(0.0, 0.0), generation_key(71))
                .unwrap();
        let second = Arc::new(snapshot_with_private_frame(0.423_456_8, 0.678_901_2));
        let second_changes =
            super::super::runtime::classify_snapshot_changes(baseline.source_snapshot(), &second);
        let second_debug = format!(
            "{:?}",
            baseline
                .project_snapshot_changes(
                    &second,
                    second_changes,
                    super::super::AppliedRevisions::new(0, 0),
                    super::super::AppliedRevisions::new(1, 1),
                )
                .unwrap()
                .frame
        );

        assert_eq!(first_debug, second_debug);
        for private_value in [0.312_345_68_f32, 0.567_890_1, 0.423_456_8, 0.678_901_2] {
            assert!(!first_debug.contains(&format!("{private_value:?}")));
            assert!(!second_debug.contains(&format!("{private_value:?}")));
        }
    }

    fn snapshot_with_private_frame(gauge: f32, dim: f32) -> CompanionSceneSnapshot {
        let mut snapshot = snapshot_for(Species::Fuzz, Stage::S3);
        snapshot.frame.gauge_fractions = [gauge; 4];
        snapshot.frame.gauge_levels =
            [super::super::GaugeLevelSnapshot::from_fraction(f64::from(gauge)); 4];
        snapshot.frame.dimmed = dim > 0.0;
        snapshot.frame.dim_amount = dim;
        snapshot
    }

    #[test]
    fn production_frame_debug_redacts_private_gauge_and_dim_state() {
        let first = build_scene_generation(
            &snapshot_with_private_frame(0.312_345_68, 0.567_890_1),
            generation_key(72),
        )
        .unwrap();
        let second = build_scene_generation(
            &snapshot_with_private_frame(0.423_456_8, 0.678_901_2),
            generation_key(72),
        )
        .unwrap();
        assert_ne!(first.frame_checksum(), second.frame_checksum());

        let first_debug = format!("{:?}", first.frame());
        let second_debug = format!("{:?}", second.frame());
        assert_eq!(first_debug, second_debug);
        let dim_node = first
            .template()
            .nodes
            .iter()
            .find(|node| node.alias.as_str() == "chrome.dim")
            .unwrap()
            .id;
        let first_node_debug = format!(
            "{:?}",
            first
                .frame()
                .nodes
                .iter()
                .find(|node| node.node == dim_node)
                .unwrap()
        );
        let second_node_debug = format!(
            "{:?}",
            second
                .frame()
                .nodes
                .iter()
                .find(|node| node.node == dim_node)
                .unwrap()
        );
        assert_eq!(first_node_debug, second_node_debug);
        for private_value in [0.312_345_68_f32, 0.567_890_1, 0.423_456_8, 0.678_901_2] {
            assert!(!first_debug.contains(&format!("{private_value:?}")));
            assert!(!second_debug.contains(&format!("{private_value:?}")));
            assert!(!first_node_debug.contains(&format!("{private_value:?}")));
            assert!(!second_node_debug.contains(&format!("{private_value:?}")));
        }
    }

    #[test]
    fn production_generation_debug_redacts_exact_frame_identity() {
        let first = build_scene_generation(
            &snapshot_with_private_frame(0.312_345_68, 0.567_890_1),
            generation_key(73),
        )
        .unwrap();
        let second = build_scene_generation(
            &snapshot_with_private_frame(0.423_456_8, 0.678_901_2),
            generation_key(73),
        )
        .unwrap();
        assert_ne!(first.frame_checksum(), second.frame_checksum());

        let first_internal_checksum = first.frame_checksum().to_string();
        let second_internal_checksum = second.frame_checksum().to_string();
        let first_debug = format!("{first:?}");
        let second_debug = format!("{second:?}");
        assert_eq!(first_debug, second_debug);
        assert!(!first_debug.contains(&first_internal_checksum));
        assert!(!second_debug.contains(&second_internal_checksum));
    }
}
