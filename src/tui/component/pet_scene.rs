use crate::tui::component::habitat_props::habitat_prop_placements_for;
use crate::tui::component::{
    ComponentPath, GeometryTarget, TargetPath, TargetRole, WatchComponentId,
};
use crate::tui::panels::pet::{pet_inner_rect_in_panel, pet_silhouette_halo_rects};
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
    pub fn compute_layout(area: Rect, vm: &WatchViewModel, _ctx: &RenderContext) -> PetSceneLayout {
        let owner = WatchComponentId::Pet.path();
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
        let pet_art = clip_rect_to_area(pet_inner_rect_in_panel(content, vm), content);
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
            None,
        );
        insert_target(
            &mut targets,
            TargetPath::new("watch.pet.habitat"),
            owner,
            habitat,
            1,
            area,
            TargetRole::Habitat,
            None,
        );
        insert_target(
            &mut targets,
            TargetPath::new("watch.pet.hit"),
            owner,
            hit_area,
            30,
            content,
            TargetRole::HitArea,
            None,
        );
        insert_target(
            &mut targets,
            TargetPath::new("watch.pet.art"),
            owner,
            pet_art,
            20,
            content,
            TargetRole::PetArt,
            None,
        );
        insert_target(
            &mut targets,
            TargetPath::new("watch.pet.effect"),
            owner,
            pet_art,
            10,
            content,
            TargetRole::Effect,
            None,
        );
        insert_target(
            &mut targets,
            TargetPath::new("watch.room.effect"),
            owner,
            habitat,
            5,
            area,
            TargetRole::RoomEffect,
            None,
        );
        if let Some(speech) = speech {
            insert_target(
                &mut targets,
                TargetPath::new("watch.pet.speech"),
                owner,
                speech,
                20,
                speech,
                TargetRole::PetSpeech,
                None,
            );
        }

        let mut effect_targets = vec![
            TargetPath::new("watch.pet.effect"),
            TargetPath::new("watch.room.effect"),
        ];

        let scene_for_placements = PetSceneLayout {
            id: owner,
            panel: area,
            speech,
            content,
            pet_art,
            hit_area,
            habitat,
            exclusions: exclusions.clone(),
            targets: BTreeMap::new(),
            effect_targets: Vec::new(),
        };
        let species = vm.pet_render.generated_species;
        let seed = &vm.pet_render.seed;
        let mirror = vm.facing == -1;
        let silhouette_halo = pet_silhouette_halo_rects(&vm.pet_art, pet_art, mirror);
        for placement in habitat_prop_placements_for(
            &vm.habitat,
            &scene_for_placements,
            &silhouette_halo,
            species,
            seed,
            _ctx,
        ) {
            if let Some(target_id) = placement.target_id {
                insert_target(
                    &mut targets,
                    target_id,
                    owner,
                    placement.bounds,
                    6,
                    area,
                    TargetRole::PropEffect,
                    Some(placement.cells.len()),
                );
                effect_targets.push(target_id);
            }
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
            effect_targets,
        }
    }
}

fn clip_rect_to_area(rect: Rect, area: Rect) -> Rect {
    if area.width == 0 || area.height == 0 {
        return Rect::new(area.x, area.y, 0, 0);
    }
    let x = rect.x.clamp(area.x, area.x.saturating_add(area.width));
    let y = rect.y.clamp(area.y, area.y.saturating_add(area.height));
    let right = rect
        .x
        .saturating_add(rect.width)
        .min(area.x.saturating_add(area.width));
    let bottom = rect
        .y
        .saturating_add(rect.height)
        .min(area.y.saturating_add(area.height));
    Rect::new(x, y, right.saturating_sub(x), bottom.saturating_sub(y))
}

fn speech_rows(vm: &WatchViewModel) -> u16 {
    if vm.current_speech.is_some() {
        1
    } else {
        0
    }
}

#[allow(clippy::too_many_arguments)]
fn insert_target(
    targets: &mut BTreeMap<TargetPath, GeometryTarget>,
    path: TargetPath,
    owner: ComponentPath,
    rect: Rect,
    z: i16,
    clip: Rect,
    role: TargetRole,
    cell_count: Option<usize>,
) {
    targets.insert(
        path,
        GeometryTarget { owner, rect, z, clip, role, cell_count },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::component::{
        hit_test, ComponentLayout, ComponentNodeLayout, TargetPath, TargetRole, WatchComponentId,
    };
    use crate::tui::render_context::{RenderContext, WatchClock};
    use crate::tui::style::ColorCapability;
    use crate::tui::view_model::WatchViewModel;
    use ratatui::layout::{Position, Rect};

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

        let scene = PetScene::compute_layout(area, &vm, &ctx);

        assert_eq!(scene.panel, area);
        assert_eq!(scene.speech, None);
        assert_eq!(scene.content, area);
        assert_eq!(scene.habitat, area);
        assert_eq!(scene.hit_area, area);
        assert_eq!(scene.pet_art, Rect::new(23, 9, 13, 10));
        assert_eq!(scene.exclusions, vec![scene.pet_art]);
        assert_eq!(
            scene.effect_targets,
            vec![
                TargetPath::new("watch.pet.effect"),
                TargetPath::new("watch.room.effect"),
            ]
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

        let scene = PetScene::compute_layout(area, &vm, &ctx);

        assert_eq!(scene.speech, Some(Rect::new(10, 5, 40, 1)));
        assert_eq!(scene.content, Rect::new(10, 6, 40, 13));
        assert_eq!(scene.hit_area, scene.content);
        assert_eq!(scene.pet_art, Rect::new(23, 9, 13, 10));
        assert_eq!(scene.exclusions, vec![scene.speech.unwrap(), scene.pet_art]);

        let speech = scene.target(TargetPath::new("watch.pet.speech")).unwrap();
        assert_eq!(speech.rect, scene.speech.unwrap());
        assert_eq!(speech.role, TargetRole::PetSpeech);
    }

    #[test]
    fn pet_scene_layout_clips_art_inside_speech_active_min_height_panel() {
        let mut vm = WatchViewModel::fixture();
        vm.current_speech = Some("hello".into());
        let ctx = test_context();
        let area = Rect::new(6, 6, 40, 10);

        let scene = PetScene::compute_layout(area, &vm, &ctx);

        assert_eq!(scene.speech, Some(Rect::new(6, 6, 40, 1)));
        assert_eq!(scene.content, Rect::new(6, 7, 40, 9));
        assert_rect_contains(scene.panel, scene.pet_art);
        assert_rect_contains(scene.content, scene.pet_art);

        for target in [
            TargetPath::new("watch.pet.art"),
            TargetPath::new("watch.pet.effect"),
        ] {
            let target = scene.target(target).unwrap();
            assert_rect_contains(scene.panel, target.rect);
            assert_rect_contains(target.clip, target.rect);
        }
    }

    #[test]
    fn pet_scene_hit_target_wins_over_art_and_effect_targets() {
        let vm = WatchViewModel::fixture();
        let ctx = test_context();
        let area = Rect::new(10, 5, 40, 14);
        let scene = PetScene::compute_layout(area, &vm, &ctx);
        let mut layout = ComponentLayout::new(area, crate::tui::component::LayoutMode::Wide);
        layout
            .insert_node(ComponentNodeLayout::leaf(
                WatchComponentId::Pet.path(),
                area,
            ))
            .unwrap();
        for (path, target) in scene.targets {
            layout.insert_target(path, target).unwrap();
        }

        let hit = hit_test(
            &layout,
            Position::new(scene.pet_art.x + 1, scene.pet_art.y + 1),
        )
        .unwrap();

        assert_eq!(hit.target, TargetPath::new("watch.pet.hit"));
    }

    #[test]
    fn pet_scene_layout_uses_stable_pet_owner_and_paths() {
        let vm = WatchViewModel::fixture();
        let ctx = test_context();
        let scene = PetScene::compute_layout(Rect::new(10, 5, 40, 14), &vm, &ctx);

        assert_eq!(scene.id, WatchComponentId::Pet.path());
        assert!(scene
            .targets
            .values()
            .all(|target| target.owner == scene.id));
    }

    fn assert_rect_contains(outer: Rect, inner: Rect) {
        assert!(
            inner.x >= outer.x
                && inner.y >= outer.y
                && inner.x.saturating_add(inner.width) <= outer.x.saturating_add(outer.width)
                && inner.y.saturating_add(inner.height) <= outer.y.saturating_add(outer.height),
            "expected {outer:?} to contain {inner:?}"
        );
    }
}
