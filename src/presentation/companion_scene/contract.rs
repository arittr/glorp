use super::scene::{
    is_world_blended, NodeId, OrthographicCamera, SceneContent, SceneFrame, SceneTemplate,
};
use super::validate::{validate_full_generation, SceneValidationError};
use super::GaugeLevelSnapshot;

pub const SCENE_ARTIFACT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SceneArtifactPrivacy {
    ExternalRedacted,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneTemplateArtifact {
    pub schema_version: u16,
    pub scene_schema_version: u16,
    pub renderer_schema_version: u16,
    pub generation_checksum: u64,
    pub privacy: SceneArtifactPrivacy,
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
    pub gauges: [GaugeLevelSnapshot; 4],
    pub dimmed: bool,
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
                primitive_count: template.primitives.len(),
                blended_draw_count: template
                    .primitives
                    .iter()
                    .filter(|primitive| {
                        let material = template
                            .materials
                            .iter()
                            .find(|material| material.id == primitive.material)
                            .map(|material| material.kind);
                        is_world_blended(primitive.blend, material)
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
                gauges: frame
                    .gauges
                    .map(|gauge| GaugeLevelSnapshot::from_fraction(f64::from(gauge))),
                dimmed: frame.dim_amount > 0.0,
                light_count: frame.lights.len(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::companion_scene::scene::{
        CanonicalAlias, MaterialKind, SceneFixture, WorldBlend,
    };

    #[test]
    fn artifact_dtos_are_versioned_deterministic_and_privacy_safe() {
        let mut fixture = SceneFixture::valid();
        fixture.template.nodes[0].alias = CanonicalAlias::new("private-node-sentinel").unwrap();
        fixture.template.nodes[0].id = NodeId::from_alias(&fixture.template.nodes[0].alias);
        fixture.template.nodes[1].parent = Some(fixture.template.nodes[0].id);
        fixture.frame.nodes[0].node = fixture.template.nodes[0].id;
        fixture.frame.gauges = [0.123_456_79, 0.234_567_9, 0.345_678_9, 0.456_789];
        fixture.frame.dim_amount = 0.567_891;
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
        for forbidden in [
            "/Users/",
            "prompt",
            "response",
            "transcript",
            "diagnostic",
            "private-node-sentinel",
            "0.12345679",
            "0.567891",
            "node_aliases",
        ] {
            assert!(
                !first_json.contains(forbidden),
                "artifact leaked {forbidden}"
            );
        }
        assert_eq!(first.frame.gauges[0], super::super::GaugeLevelSnapshot::Low);
        assert!(!format!("{:?}", fixture.template).contains("private-node-sentinel"));
        assert!(!format!("{:?}", fixture.frame).contains("0.12345679"));
        assert!(!format!("{:?}", fixture.frame).contains("0.567891"));
    }

    #[test]
    fn screen_chrome_is_not_counted_as_a_world_blended_draw() {
        let mut fixture = SceneFixture::valid();
        fixture.template.materials[0].kind = MaterialKind::ScreenChrome;
        fixture.template.primitives[0].blend = WorldBlend::PremultipliedAlpha;
        fixture.template.primitives[0].depth = super::super::scene::DepthBehavior::ScreenNoDepth;
        let artifacts =
            SceneArtifacts::try_from_parts(&fixture.template, &fixture.content, &fixture.frame)
                .unwrap();
        assert_eq!(artifacts.template.blended_draw_count, 0);
    }

    #[test]
    fn material_aware_blended_limit_and_artifact_count_agree() {
        let mut chrome = SceneFixture::valid();
        chrome.template.materials[0].kind = MaterialKind::ScreenChrome;
        chrome.template.primitives[0].blend = WorldBlend::PremultipliedAlpha;
        chrome.template.primitives[0].depth = super::super::scene::DepthBehavior::ScreenNoDepth;
        chrome.template.primitives = vec![chrome.template.primitives[0].clone(); 257];
        assert!(super::super::validate::validate_template(&chrome.template).is_ok());
        let chrome_artifacts =
            SceneArtifacts::try_from_parts(&chrome.template, &chrome.content, &chrome.frame)
                .unwrap();
        assert_eq!(chrome_artifacts.template.blended_draw_count, 0);

        let mut world = SceneFixture::valid();
        world.template.primitives[0].blend = WorldBlend::PremultipliedAlpha;
        world.template.primitives[0].depth = super::super::scene::DepthBehavior::WorldReadOnly;
        world.template.primitives = vec![world.template.primitives[0].clone(); 257];
        assert_eq!(
            super::super::validate::validate_template(&world.template),
            Err(SceneValidationError::BlendedDrawCapacityExceeded)
        );
        world.template.primitives.pop();
        let world_artifacts =
            SceneArtifacts::try_from_parts(&world.template, &world.content, &world.frame).unwrap();
        assert_eq!(world_artifacts.template.blended_draw_count, 256);
    }
}
