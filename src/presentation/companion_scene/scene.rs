use crate::presentation::privacy::PrivacyProjection;
use std::ops::Mul;

pub const SCENE_CONTRACT_SCHEMA_VERSION: u16 = super::COMPANION_SCENE_SCHEMA_VERSION;
pub const MAX_SCENE_NODES: usize = 128;
pub const MAX_STATIC_PRIMITIVES: usize = 768;
pub const MAX_PET_ART_SLOTS: usize = 130;
pub const MAX_VISIBLE_PROPS: usize = 10;
pub const MAX_ROUND_TANK_INHABITANTS: usize = 2;
pub const MAX_AMBIENT_INSTANCES: usize = 64;
pub const MAX_BLENDED_DRAWS: usize = 256;
pub const MAX_LIGHTS: usize = 2;
pub const MAX_ATTACHMENTS: usize = 32;
pub const LIT_CARD_SCALE_TOLERANCE: f32 = 1.0e-5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliasError {
    Empty,
    NonCanonicalAscii,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(transparent)]
pub struct CanonicalAlias(String);

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
            .map(|component| component * component)
            .sum::<f32>();
        if length_squared <= f32::EPSILON {
            return Err(TransformError::ZeroQuaternion);
        }
        let inverse_length = length_squared.sqrt().recip();
        let [x, y, z, w] = value.map(|component| component * inverse_length);
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

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct DepthCue {
    pub scale: f32,
    pub y_offset_points_up: f32,
    pub opacity: f32,
    pub saturation: f32,
}

impl DepthCue {
    pub const NEUTRAL: Self = Self {
        scale: 1.0,
        y_offset_points_up: 0.0,
        opacity: 1.0,
        saturation: 1.0,
    };
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
        Ok(Self {
            width_points,
            height_points,
            far_z,
            near_z,
        })
    }

    pub fn clip_depth(self, world_z: f32) -> f32 {
        (self.near_z - world_z) / (self.near_z - self.far_z)
    }

    pub fn projection_matrix(self) -> Mat4 {
        let depth_range = self.near_z - self.far_z;
        Mat4 {
            columns: [
                [2.0 / self.width_points, 0.0, 0.0, 0.0],
                [0.0, 2.0 / self.height_points, 0.0, 0.0],
                [0.0, 0.0, -1.0 / depth_range, 0.0],
                [-1.0, -1.0, self.near_z / depth_range, 1.0],
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Bounds3 {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct NodeTemplate {
    pub id: NodeId,
    pub alias: CanonicalAlias,
    pub parent: Option<NodeId>,
    pub base_transform: Transform3,
    pub local_bounds: Bounds3,
    pub depth_cue: DepthCue,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct MaterialTemplate {
    pub id: MaterialId,
    pub alias: CanonicalAlias,
    pub kind: MaterialKind,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
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
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AttachmentTemplate {
    pub id: AttachmentId,
    pub alias: CanonicalAlias,
    pub owner: NodeId,
    pub local: Transform3,
    pub mode: AttachmentMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct SceneCapacities {
    pub max_nodes: usize,
    pub max_static_primitives: usize,
    pub max_pet_art_slots: usize,
    pub max_visible_props: usize,
    pub max_round_tank_inhabitants: usize,
    pub max_ambient_instances: usize,
    pub max_blended_draws: usize,
    pub max_lights: usize,
    pub max_attachments: usize,
}

impl SceneCapacities {
    pub const FIXED_V1: Self = Self {
        max_nodes: MAX_SCENE_NODES,
        max_static_primitives: MAX_STATIC_PRIMITIVES,
        max_pet_art_slots: MAX_PET_ART_SLOTS,
        max_visible_props: MAX_VISIBLE_PROPS,
        max_round_tank_inhabitants: MAX_ROUND_TANK_INHABITANTS,
        max_ambient_instances: MAX_AMBIENT_INSTANCES,
        max_blended_draws: MAX_BLENDED_DRAWS,
        max_lights: MAX_LIGHTS,
        max_attachments: MAX_ATTACHMENTS,
    };
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneTemplate {
    pub schema_version: u16,
    pub renderer_schema_version: u16,
    pub capacities: SceneCapacities,
    pub nodes: Vec<NodeTemplate>,
    pub primitives: Vec<PrimitiveTemplate>,
    pub materials: Vec<MaterialTemplate>,
    pub resources: Vec<ResourceTemplate>,
    pub attachments: Vec<AttachmentTemplate>,
    pub privacy: PrivacyProjection,
    pub generation_checksum: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PetArtSlot {
    pub slot: u16,
    pub glyph: Option<char>,
    pub palette_role: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PropContentSlot {
    pub slot: u8,
    pub state: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct TankContentSlot {
    pub slot: u8,
    pub state: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct AmbientContentSlot {
    pub slot: u8,
    pub active: bool,
    pub kind: u8,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneContent {
    pub schema_version: u16,
    pub renderer_schema_version: u16,
    pub pet_art_slots: Vec<PetArtSlot>,
    pub prop_slots: Vec<PropContentSlot>,
    pub tank_slots: Vec<TankContentSlot>,
    pub ambient_slots: Vec<AmbientContentSlot>,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct NodeFrameState {
    pub node: NodeId,
    pub local_transform: Transform3,
    pub visible: bool,
    pub opacity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct LightFrame {
    pub direction: [f32; 3],
    pub color_linear: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneFrame {
    pub schema_version: u16,
    pub renderer_schema_version: u16,
    pub camera: OrthographicCamera,
    pub nodes: Vec<NodeFrameState>,
    pub gauges: [f32; 4],
    pub dim_amount: f32,
    pub lights: Vec<LightFrame>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ContentDelta {
    pub schema_version: u16,
    pub renderer_schema_version: u16,
    pub pet_art_slots: Vec<PetArtSlot>,
    pub prop_slots: Vec<PropContentSlot>,
    pub tank_slots: Vec<TankContentSlot>,
    pub ambient_slots: Vec<AmbientContentSlot>,
}

impl ContentDelta {
    pub const fn empty() -> Self {
        Self {
            schema_version: SCENE_CONTRACT_SCHEMA_VERSION,
            renderer_schema_version: super::COMPANION_RENDERER_SCHEMA_VERSION,
            pet_art_slots: Vec::new(),
            prop_slots: Vec::new(),
            tank_slots: Vec::new(),
            ambient_slots: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FrameDelta {
    pub schema_version: u16,
    pub renderer_schema_version: u16,
    pub camera: Option<OrthographicCamera>,
    pub nodes: Vec<NodeFrameState>,
    pub gauges: Option<[f32; 4]>,
    pub dim_amount: Option<f32>,
    pub lights: Vec<(u8, LightFrame)>,
}

impl FrameDelta {
    pub const fn empty() -> Self {
        Self {
            schema_version: SCENE_CONTRACT_SCHEMA_VERSION,
            renderer_schema_version: super::COMPANION_RENDERER_SCHEMA_VERSION,
            camera: None,
            nodes: Vec::new(),
            gauges: None,
            dim_amount: None,
            lights: Vec::new(),
        }
    }
}

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
        let attachment_alias = alias("pet.body.bubble-origin");
        let camera = OrthographicCamera::new(360.0, 360.0, -2.0, 2.0).unwrap();
        Self {
            template: SceneTemplate {
                schema_version: SCENE_CONTRACT_SCHEMA_VERSION,
                renderer_schema_version: super::COMPANION_RENDERER_SCHEMA_VERSION,
                capacities: SceneCapacities::FIXED_V1,
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
                }],
                privacy: PrivacyProjection::for_surface(
                    crate::presentation::privacy::PresentationSurface::RoundCompanion,
                ),
                generation_checksum: 1,
            },
            content: SceneContent {
                schema_version: SCENE_CONTRACT_SCHEMA_VERSION,
                renderer_schema_version: super::COMPANION_RENDERER_SCHEMA_VERSION,
                pet_art_slots: vec![PetArtSlot {
                    slot: 0,
                    glyph: Some('@'),
                    palette_role: 0,
                }],
                prop_slots: vec![],
                tank_slots: vec![],
                ambient_slots: vec![],
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
        template.resources[0].kind = ResourceKind::ShallowCardGeometry;
        template.primitives[0].blend = WorldBlend::Opaque;
        template
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1.0e-5;

    fn assert_point_close(actual: [f32; 4], expected: [f32; 4]) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= EPSILON,
                "{actual} != {expected}"
            );
        }
    }

    #[test]
    fn fixed_v1_capacities_are_exact() {
        assert_eq!(MAX_SCENE_NODES, 128);
        assert_eq!(MAX_STATIC_PRIMITIVES, 768);
        assert_eq!(MAX_PET_ART_SLOTS, 130);
        assert_eq!(MAX_VISIBLE_PROPS, 10);
        assert_eq!(MAX_ROUND_TANK_INHABITANTS, 2);
        assert_eq!(MAX_AMBIENT_INSTANCES, 64);
        assert_eq!(MAX_BLENDED_DRAWS, 256);
        assert_eq!(MAX_LIGHTS, 2);
        assert_eq!(MAX_ATTACHMENTS, 32);
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
    fn orthographic_depth_maps_near_to_zero_and_far_to_one() {
        let camera = OrthographicCamera::new(360.0, 360.0, -2.0, 2.0).unwrap();
        assert_eq!(camera.clip_depth(2.0), 0.0);
        assert_eq!(camera.clip_depth(-2.0), 1.0);
        assert_eq!(camera.clip_depth(0.0), 0.5);
        let matrix = camera.projection_matrix();
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
    }
}
