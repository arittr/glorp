use super::scene::{
    AttachmentId, CanonicalAlias, MaterialId, NodeId, OrthographicCamera, ResourceId, SceneContent,
    SceneFrame, SceneTemplate, WorldBlend,
};
use super::validate::{validate_full_generation, SceneValidationError};

pub const SCENE_ARTIFACT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SceneArtifactPrivacy {
    ExternalRedacted,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct NodeAliasArtifact {
    pub id: NodeId,
    pub alias: CanonicalAlias,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AttachmentAliasArtifact {
    pub id: AttachmentId,
    pub alias: CanonicalAlias,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MaterialAliasArtifact {
    pub id: MaterialId,
    pub alias: CanonicalAlias,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResourceAliasArtifact {
    pub id: ResourceId,
    pub alias: CanonicalAlias,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneTemplateArtifact {
    pub schema_version: u16,
    pub scene_schema_version: u16,
    pub renderer_schema_version: u16,
    pub generation_checksum: u64,
    pub privacy: SceneArtifactPrivacy,
    pub node_aliases: Vec<NodeAliasArtifact>,
    pub attachment_aliases: Vec<AttachmentAliasArtifact>,
    pub material_aliases: Vec<MaterialAliasArtifact>,
    pub resource_aliases: Vec<ResourceAliasArtifact>,
    pub primitive_count: usize,
    pub blended_draw_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SceneContentArtifact {
    pub schema_version: u16,
    pub occupied_pet_art_slots: Vec<u16>,
    pub occupied_prop_slots: Vec<u8>,
    pub occupied_tank_slots: Vec<u8>,
    pub active_ambient_slots: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneFrameNodeArtifact {
    pub node: NodeId,
    pub visible: bool,
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneFrameArtifact {
    pub schema_version: u16,
    pub camera: OrthographicCamera,
    pub nodes: Vec<SceneFrameNodeArtifact>,
    pub gauges: [f32; 4],
    pub dim_amount: f32,
    pub light_count: usize,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneArtifacts {
    pub schema_version: u16,
    pub template: SceneTemplateArtifact,
    pub content: SceneContentArtifact,
    pub frame: SceneFrameArtifact,
}

impl SceneArtifacts {
    pub fn try_from_parts(
        template: &SceneTemplate,
        content: &SceneContent,
        frame: &SceneFrame,
    ) -> Result<Self, SceneValidationError> {
        validate_full_generation(template, content, frame)?;

        let mut node_aliases = template
            .nodes
            .iter()
            .map(|node| NodeAliasArtifact { id: node.id, alias: node.alias.clone() })
            .collect::<Vec<_>>();
        node_aliases.sort_by(|left, right| left.alias.cmp(&right.alias));
        let mut attachment_aliases = template
            .attachments
            .iter()
            .map(|attachment| AttachmentAliasArtifact {
                id: attachment.id,
                alias: attachment.alias.clone(),
            })
            .collect::<Vec<_>>();
        attachment_aliases.sort_by(|left, right| left.alias.cmp(&right.alias));
        let mut material_aliases = template
            .materials
            .iter()
            .map(|material| MaterialAliasArtifact {
                id: material.id,
                alias: material.alias.clone(),
            })
            .collect::<Vec<_>>();
        material_aliases.sort_by(|left, right| left.alias.cmp(&right.alias));
        let mut resource_aliases = template
            .resources
            .iter()
            .map(|resource| ResourceAliasArtifact {
                id: resource.id,
                alias: resource.alias.clone(),
            })
            .collect::<Vec<_>>();
        resource_aliases.sort_by(|left, right| left.alias.cmp(&right.alias));

        let mut occupied_pet_art_slots = content
            .pet_art_slots
            .iter()
            .filter_map(|slot| slot.glyph.map(|_| slot.slot))
            .collect::<Vec<_>>();
        occupied_pet_art_slots.sort_unstable();
        let mut occupied_prop_slots = content
            .prop_slots
            .iter()
            .map(|slot| slot.slot)
            .collect::<Vec<_>>();
        occupied_prop_slots.sort_unstable();
        let mut occupied_tank_slots = content
            .tank_slots
            .iter()
            .map(|slot| slot.slot)
            .collect::<Vec<_>>();
        occupied_tank_slots.sort_unstable();
        let mut active_ambient_slots = content
            .ambient_slots
            .iter()
            .filter(|slot| slot.active)
            .map(|slot| slot.slot)
            .collect::<Vec<_>>();
        active_ambient_slots.sort_unstable();
        let mut nodes = frame
            .nodes
            .iter()
            .map(|node| SceneFrameNodeArtifact {
                node: node.node,
                visible: node.visible,
                opacity: node.opacity,
            })
            .collect::<Vec<_>>();
        nodes.sort_by_key(|node| node.node);

        Ok(Self {
            schema_version: SCENE_ARTIFACT_SCHEMA_VERSION,
            template: SceneTemplateArtifact {
                schema_version: SCENE_ARTIFACT_SCHEMA_VERSION,
                scene_schema_version: template.schema_version,
                renderer_schema_version: template.renderer_schema_version,
                generation_checksum: template.generation_checksum,
                privacy: SceneArtifactPrivacy::ExternalRedacted,
                node_aliases,
                attachment_aliases,
                material_aliases,
                resource_aliases,
                primitive_count: template.primitives.len(),
                blended_draw_count: template
                    .primitives
                    .iter()
                    .filter(|primitive| {
                        matches!(
                            primitive.blend,
                            WorldBlend::PremultipliedAlpha
                                | WorldBlend::Multiply
                                | WorldBlend::Additive
                        )
                    })
                    .count(),
            },
            content: SceneContentArtifact {
                schema_version: SCENE_ARTIFACT_SCHEMA_VERSION,
                occupied_pet_art_slots,
                occupied_prop_slots,
                occupied_tank_slots,
                active_ambient_slots,
            },
            frame: SceneFrameArtifact {
                schema_version: SCENE_ARTIFACT_SCHEMA_VERSION,
                camera: frame.camera,
                nodes,
                gauges: frame.gauges,
                dim_amount: frame.dim_amount,
                light_count: frame.lights.len(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::companion_scene::scene::SceneFixture;

    #[test]
    fn artifact_dtos_are_versioned_deterministic_and_privacy_safe() {
        let fixture = SceneFixture::valid();
        let first =
            SceneArtifacts::try_from_parts(&fixture.template, &fixture.content, &fixture.frame)
                .unwrap();
        let second =
            SceneArtifacts::try_from_parts(&fixture.template, &fixture.content, &fixture.frame)
                .unwrap();
        let first_json = serde_json::to_string(&first).unwrap();
        let second_json = serde_json::to_string(&second).unwrap();
        assert_eq!(first_json, second_json);
        assert!(first_json.contains("\"schema_version\":1"));
        for forbidden in ["/Users/", "prompt", "response", "transcript", "diagnostic"] {
            assert!(
                !first_json.contains(forbidden),
                "artifact leaked {forbidden}"
            );
        }
    }
}
