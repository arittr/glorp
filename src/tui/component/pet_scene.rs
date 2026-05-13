use crate::tui::component::{
    ComponentPath, GeometryTarget, TargetPath, TargetRole, WatchComponentId,
};
use crate::tui::panels::pet::pet_inner_rect_in_panel;
use crate::tui::render_context::RenderContext;
use crate::tui::view_model::WatchViewModel;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetSceneLayout {
    pub id: ComponentPath,
    pub panel: Rect,
    pub speech: Option<Rect>,
    pub content: Rect,
    pub pet_art: Rect,
    pub hit_area: Rect,
    pub habitat: Rect,
    pub exclusions: Vec<Rect>,
    pub targets: BTreeMap<TargetPath, GeometryTarget>,
    pub effect_targets: Vec<TargetPath>,
}

impl PetSceneLayout {
    pub fn target(&self, path: TargetPath) -> Option<&GeometryTarget> {
        self.targets.get(&path)
    }
}

pub struct PetScene;

impl PetScene {
    pub fn compute_layout(
        id: WatchComponentId,
        area: Rect,
        vm: &WatchViewModel,
        _ctx: &RenderContext,
    ) -> PetSceneLayout {
        let owner = id.path();
        let speech_h = speech_rows(vm).min(area.height);
        let speech = if speech_h > 0 {
            Some(Rect::new(area.x, area.y, area.width, speech_h))
        } else {
            None
        };
        let content = Rect::new(
            area.x,
            area.y + speech_h,
            area.width,
            area.height.saturating_sub(speech_h),
        );
        let pet_art = pet_inner_rect_in_panel(content, vm);
        let habitat = area;
        let hit_area = content;
        let mut exclusions = Vec::new();
        if let Some(speech) = speech {
            exclusions.push(speech);
        }
        exclusions.push(pet_art);

        let mut targets = BTreeMap::new();
        insert_target(
            &mut targets,
            TargetPath::new("watch.pet.panel"),
            owner,
            area,
            0,
            area,
            TargetRole::PetPanel,
        );
        insert_target(
            &mut targets,
            TargetPath::new("watch.pet.habitat"),
            owner,
            habitat,
            1,
            area,
            TargetRole::Habitat,
        );
        insert_target(
            &mut targets,
            TargetPath::new("watch.pet.hit"),
            owner,
            hit_area,
            5,
            area,
            TargetRole::HitArea,
        );
        insert_target(
            &mut targets,
            TargetPath::new("watch.pet.art"),
            owner,
            pet_art,
            10,
            area,
            TargetRole::PetArt,
        );
        insert_target(
            &mut targets,
            TargetPath::new("watch.pet.effect"),
            owner,
            pet_art,
            10,
            area,
            TargetRole::Effect,
        );
        if let Some(speech) = speech {
            insert_target(
                &mut targets,
                TargetPath::new("watch.pet.speech"),
                owner,
                speech,
                20,
                area,
                TargetRole::PetSpeech,
            );
        }

        PetSceneLayout {
            id: owner,
            panel: area,
            speech,
            content,
            pet_art,
            hit_area,
            habitat,
            exclusions,
            targets,
            effect_targets: vec![TargetPath::new("watch.pet.effect")],
        }
    }
}

fn speech_rows(vm: &WatchViewModel) -> u16 {
    if vm.current_speech.is_some() {
        1
    } else {
        0
    }
}

fn insert_target(
    targets: &mut BTreeMap<TargetPath, GeometryTarget>,
    path: TargetPath,
    owner: ComponentPath,
    rect: Rect,
    z: i16,
    clip: Rect,
    role: TargetRole,
) {
    targets.insert(
        path,
        GeometryTarget {
            owner,
            rect,
            z,
            clip,
            role,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::component::{TargetPath, TargetRole, WatchComponentId};
    use crate::tui::render_context::{RenderContext, WatchClock};
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::WatchViewModel;
    use ratatui::layout::Rect;

    fn test_context() -> RenderContext {
        RenderContext::with_clock(
            ColorCapability::Truecolor,
            WatchClock::fixed(time::OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap()),
        )
    }

    #[test]
    fn pet_scene_layout_centers_art_and_exports_targets_without_speech() {
        let vm = WatchViewModel::fixture();
        let ctx = test_context();
        let area = Rect::new(10, 5, 40, 14);

        let scene = PetScene::compute_layout(WatchComponentId::Pet, area, &vm, &ctx);

        assert_eq!(scene.panel, area);
        assert_eq!(scene.speech, None);
        assert_eq!(scene.content, area);
        assert_eq!(scene.habitat, area);
        assert_eq!(scene.hit_area, area);
        assert_eq!(scene.pet_art, Rect::new(23, 7, 13, 10));
        assert_eq!(scene.exclusions, vec![scene.pet_art]);
        assert_eq!(
            scene.effect_targets,
            vec![TargetPath::new("watch.pet.effect")]
        );

        let panel = scene.target(TargetPath::new("watch.pet.panel")).unwrap();
        assert_eq!(panel.rect, area);
        assert_eq!(panel.role, TargetRole::PetPanel);

        let art = scene.target(TargetPath::new("watch.pet.art")).unwrap();
        assert_eq!(art.rect, scene.pet_art);
        assert_eq!(art.role, TargetRole::PetArt);

        let effect = scene.target(TargetPath::new("watch.pet.effect")).unwrap();
        assert_eq!(effect.rect, scene.pet_art);
        assert_eq!(effect.role, TargetRole::Effect);

        let habitat = scene.target(TargetPath::new("watch.pet.habitat")).unwrap();
        assert_eq!(habitat.rect, scene.habitat);
        assert_eq!(habitat.role, TargetRole::Habitat);

        let hit = scene.target(TargetPath::new("watch.pet.hit")).unwrap();
        assert_eq!(hit.rect, scene.hit_area);
        assert_eq!(hit.role, TargetRole::HitArea);
    }

    #[test]
    fn pet_scene_layout_reserves_optional_speech_above_content() {
        let mut vm = WatchViewModel::fixture();
        vm.current_speech = Some("hello".into());
        let ctx = test_context();
        let area = Rect::new(10, 5, 40, 14);

        let scene = PetScene::compute_layout(WatchComponentId::Pet, area, &vm, &ctx);

        assert_eq!(scene.speech, Some(Rect::new(10, 5, 40, 1)));
        assert_eq!(scene.content, Rect::new(10, 6, 40, 13));
        assert_eq!(scene.hit_area, scene.content);
        assert_eq!(scene.pet_art, Rect::new(23, 7, 13, 10));
        assert_eq!(scene.exclusions, vec![scene.speech.unwrap(), scene.pet_art]);

        let speech = scene.target(TargetPath::new("watch.pet.speech")).unwrap();
        assert_eq!(speech.rect, scene.speech.unwrap());
        assert_eq!(speech.role, TargetRole::PetSpeech);
    }
}
