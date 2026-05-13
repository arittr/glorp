use crate::tui::component::{
    ComponentLayout, ComponentNodeLayout, ComponentPath, LayoutDecision, LayoutDecisionReason,
    LayoutMode, PetScene, TargetPath, VisibilityState, WatchComponentId,
};
use crate::tui::panels::{
    BioCardPanel, FeedPanel, Panel as LegacyPanel, PetPanel, ProgressPanel, TodayPanel, VitalsPanel,
};
use crate::tui::render_context::RenderContext;
use crate::tui::style::ColorCapability;
use crate::tui::view_model::WatchViewModel;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Flex, Layout, Rect};
use ratatui::widgets::Block;

/// Smallest terminal width that uses the wide two-column layout.
/// Below this threshold we fall back to the single-column compact layout.
pub const COMPACT_THRESHOLD: usize = 104;

/// Left column width in the wide layout (pet + vitals column).
pub const WIDE_LEFT_COL: u16 = 40;

/// Gutter width between left and right columns in the wide layout.
pub const WIDE_GUTTER: u16 = 4;

/// Maximum frame dimensions. Mirrors the current watch layout exactly for
/// the adapter phase.
pub const MAX_FRAME_WIDTH: u16 = 110;
pub const MAX_FRAME_HEIGHT: u16 = 23;

/// Vertical padding inside the rounded frame, between the chrome border and
/// the first/last panel.
pub const INNER_VPAD: u16 = 1;

/// Current wide-mode row bands.
pub const WIDE_BAND_1: u16 = 10;
pub const WIDE_BAND_2: u16 = 8;

/// Gap between stacked panels in both wide and compact layouts.
pub const COLUMN_GAP: u16 = 1;

pub const RIGHT_MIN_WIDTH: u16 = 50;

/// Current renderer only draws the pet panel when the allocated area can hold
/// the 13x10 pet art.
pub const PET_RENDER_MIN_HEIGHT: u16 = 10;

pub fn layout_watch(terminal_area: Rect, vm: &WatchViewModel) -> ComponentLayout {
    let ctx = RenderContext::new(ColorCapability::Truecolor);
    layout_watch_with_context(terminal_area, vm, &ctx)
}

pub fn layout_watch_with_context(
    terminal_area: Rect,
    vm: &WatchViewModel,
    ctx: &RenderContext,
) -> ComponentLayout {
    let frame = bounded_frame_rect(terminal_area);
    let mode = if (frame.width as usize) >= COMPACT_THRESHOLD + 2 {
        LayoutMode::Wide
    } else {
        LayoutMode::Compact
    };
    let content = Block::bordered().inner(frame);
    let mut layout = ComponentLayout::new(frame, mode).with_content(content);
    insert_root_node(&mut layout, frame, content);

    match mode {
        LayoutMode::Wide => layout_wide(content, vm, ctx, &mut layout),
        LayoutMode::Compact => layout_compact(content, vm, ctx, &mut layout),
    }

    layout
}

pub fn render_watch_layout(
    layout: &ComponentLayout,
    buf: &mut Buffer,
    vm: &WatchViewModel,
    ctx: &RenderContext,
) {
    render_if_visible(
        layout,
        WatchComponentId::Pet.path(),
        buf,
        vm,
        ctx,
        PetPanel,
        Some(TargetPath::new("watch.pet.art")),
    );
    render_if_visible(
        layout,
        WatchComponentId::Vitals.path(),
        buf,
        vm,
        ctx,
        VitalsPanel,
        None,
    );
    render_if_visible(
        layout,
        WatchComponentId::Bio.path(),
        buf,
        vm,
        ctx,
        BioCardPanel,
        None,
    );
    render_if_visible(
        layout,
        WatchComponentId::Today.path(),
        buf,
        vm,
        ctx,
        TodayPanel,
        None,
    );
    render_if_visible(
        layout,
        WatchComponentId::Progress.path(),
        buf,
        vm,
        ctx,
        ProgressPanel,
        None,
    );
    render_if_visible(
        layout,
        WatchComponentId::Feed.path(),
        buf,
        vm,
        ctx,
        FeedPanel,
        None,
    );
}

fn render_if_visible<P: LegacyPanel>(
    layout: &ComponentLayout,
    id: ComponentPath,
    buf: &mut Buffer,
    vm: &WatchViewModel,
    ctx: &RenderContext,
    panel: P,
    required_target: Option<TargetPath>,
) {
    let Some(node) = layout.node(id) else {
        return;
    };
    if required_target
        .map(|target| layout.target(target).is_none())
        .unwrap_or(false)
    {
        return;
    }
    if matches!(
        node.visibility,
        VisibilityState::Visible | VisibilityState::Degraded { .. }
    ) {
        panel.render(node.bounds, buf, vm, ctx);
    }
}

/// Returns a sub-rect of `terminal_area` that is at most the current watch
/// frame cap, centered within the terminal.
pub fn bounded_frame_rect(terminal_area: Rect) -> Rect {
    let width = terminal_area.width.min(MAX_FRAME_WIDTH);
    let is_wide = (width as usize) >= COMPACT_THRESHOLD + 2;
    let height = if is_wide {
        terminal_area.height.min(MAX_FRAME_HEIGHT)
    } else {
        terminal_area.height
    };
    let x = terminal_area.x + terminal_area.width.saturating_sub(width) / 2;
    let y = terminal_area.y + terminal_area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

fn layout_wide(area: Rect, vm: &WatchViewModel, ctx: &RenderContext, layout: &mut ComponentLayout) {
    let padded = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(INNER_VPAD),
            Constraint::Min(0),
            Constraint::Length(INNER_VPAD),
        ])
        .split(area)[1];

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(WIDE_LEFT_COL),
            Constraint::Length(WIDE_GUTTER),
            Constraint::Min(RIGHT_MIN_WIDTH),
        ])
        .split(padded);

    let band_constraints = [
        Constraint::Length(WIDE_BAND_1),
        Constraint::Length(COLUMN_GAP),
        Constraint::Length(WIDE_BAND_2),
        Constraint::Min(0),
    ];
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints(band_constraints)
        .split(body[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints(band_constraints)
        .split(body[2]);

    insert_pet_node(layout, left[0], vm, ctx);

    let right_top = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            TodayPanel.preferred_constraint(vm),
            Constraint::Length(COLUMN_GAP),
            Constraint::Min(0),
            ProgressPanel.preferred_constraint(vm),
        ])
        .split(right[0]);
    insert_visible_node(layout, WatchComponentId::Today, right_top[0]);
    insert_visible_node(layout, WatchComponentId::Progress, right_top[3]);

    let left_bottom = Layout::default()
        .direction(Direction::Vertical)
        .flex(Flex::Start)
        .constraints([
            VitalsPanel.preferred_constraint(vm),
            Constraint::Length(COLUMN_GAP),
            BioCardPanel.preferred_constraint(vm),
        ])
        .split(left[2]);
    insert_visible_node(layout, WatchComponentId::Vitals, left_bottom[0]);
    insert_visible_node(layout, WatchComponentId::Bio, left_bottom[2]);

    insert_visible_node(layout, WatchComponentId::Feed, right[2]);
}

fn layout_compact(
    area: Rect,
    vm: &WatchViewModel,
    ctx: &RenderContext,
    layout: &mut ComponentLayout,
) {
    let stack = Layout::default()
        .direction(Direction::Vertical)
        .flex(Flex::Start)
        .constraints([
            PetPanel.preferred_constraint(vm),
            VitalsPanel.preferred_constraint(vm),
            Constraint::Length(COLUMN_GAP),
            TodayPanel.preferred_constraint(vm),
            Constraint::Length(COLUMN_GAP),
            ProgressPanel.preferred_constraint(vm),
            Constraint::Length(COLUMN_GAP),
            FeedPanel.preferred_constraint(vm),
        ])
        .split(area);

    insert_pet_node(layout, stack[0], vm, ctx);
    insert_visible_node(layout, WatchComponentId::Vitals, stack[1]);
    insert_visible_node(layout, WatchComponentId::Today, stack[3]);
    insert_visible_node(layout, WatchComponentId::Progress, stack[5]);
    insert_visible_node(layout, WatchComponentId::Feed, stack[7]);
    insert_hidden_bio(layout, area);
}

fn insert_root_node(layout: &mut ComponentLayout, frame: Rect, content: Rect) {
    let mut root = ComponentNodeLayout::leaf(WatchComponentId::Root.path(), frame);
    root.content = content;
    root.children = vec![
        WatchComponentId::Pet.path(),
        WatchComponentId::Vitals.path(),
        WatchComponentId::Bio.path(),
        WatchComponentId::Today.path(),
        WatchComponentId::Progress.path(),
        WatchComponentId::Feed.path(),
    ];
    layout
        .insert_node(root)
        .expect("root component id is unique in watch layout");
}

fn insert_visible_node(layout: &mut ComponentLayout, id: WatchComponentId, bounds: Rect) {
    layout
        .insert_node(ComponentNodeLayout::leaf(id.path(), bounds))
        .expect("component id is unique in watch layout");
}

fn insert_pet_node(
    layout: &mut ComponentLayout,
    bounds: Rect,
    vm: &WatchViewModel,
    ctx: &RenderContext,
) {
    let owner = WatchComponentId::Pet.path();
    let mut node = ComponentNodeLayout::leaf(owner, bounds);
    if bounds.height < PET_RENDER_MIN_HEIGHT {
        node.visibility = VisibilityState::Degraded {
            reason: LayoutDecisionReason::InsufficientHeight,
        };
        layout
            .insert_node(node)
            .expect("pet component id is unique in watch layout");
        layout.decisions.push(LayoutDecision {
            path: owner,
            reason: LayoutDecisionReason::InsufficientHeight,
            message: "pet degraded because allocated height is below renderer minimum",
        });
        return;
    }

    layout
        .insert_node(node)
        .expect("pet component id is unique in watch layout");

    let scene = PetScene::compute_layout(bounds, vm, ctx);
    for (path, target) in scene.targets {
        layout
            .insert_target(path, target)
            .expect("pet scene target id is unique");
    }
}

fn insert_hidden_bio(layout: &mut ComponentLayout, content: Rect) {
    let bounds = Rect::new(content.x, content.y, 0, 0);
    let mut bio = ComponentNodeLayout::leaf(WatchComponentId::Bio.path(), bounds);
    bio.visibility = VisibilityState::Hidden {
        reason: LayoutDecisionReason::CompactMode,
    };
    layout
        .insert_node(bio)
        .expect("bio component id is unique in watch layout");
    layout.decisions.push(LayoutDecision {
        path: WatchComponentId::Bio.path(),
        reason: LayoutDecisionReason::CompactMode,
        message: "bio hidden in compact mode",
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::component::{
        LayoutDecisionReason, LayoutMode, TargetPath, VisibilityState, WatchComponentId,
    };
    use crate::tui::view_model::WatchViewModel;
    use ratatui::layout::Rect;

    #[test]
    fn layout_watch_wide_exports_required_nodes_and_pet_targets() {
        let vm = WatchViewModel::fixture();
        let layout = layout_watch(Rect::new(0, 0, 120, 32), &vm);

        assert_eq!(layout.mode, LayoutMode::Wide);
        assert_eq!(layout.frame, Rect::new(5, 4, 110, 23));
        assert_eq!(layout.content, Rect::new(6, 5, 108, 21));
        for id in [
            WatchComponentId::Root,
            WatchComponentId::Pet,
            WatchComponentId::Vitals,
            WatchComponentId::Bio,
            WatchComponentId::Today,
            WatchComponentId::Progress,
            WatchComponentId::Feed,
        ] {
            assert!(
                layout.node(id.path()).is_some(),
                "missing node {}",
                id.path()
            );
        }

        assert_eq!(
            layout.node(WatchComponentId::Pet.path()).unwrap().bounds,
            Rect::new(6, 6, 40, 10)
        );
        assert_eq!(
            layout.node(WatchComponentId::Vitals.path()).unwrap().bounds,
            Rect::new(6, 17, 40, 4)
        );
        assert_eq!(
            layout.node(WatchComponentId::Bio.path()).unwrap().bounds,
            Rect::new(6, 22, 40, 3)
        );
        assert_eq!(
            layout.node(WatchComponentId::Today.path()).unwrap().bounds,
            Rect::new(50, 6, 64, 6)
        );
        assert_eq!(
            layout
                .node(WatchComponentId::Progress.path())
                .unwrap()
                .bounds,
            Rect::new(50, 14, 64, 2)
        );
        assert_eq!(
            layout.node(WatchComponentId::Feed.path()).unwrap().bounds,
            Rect::new(50, 17, 64, 8)
        );

        let pet_panel = layout.target(TargetPath::new("watch.pet.panel")).unwrap();
        assert_eq!(pet_panel.rect, Rect::new(6, 6, 40, 10));

        let pet_node = layout.node(WatchComponentId::Pet.path()).unwrap();
        assert!(pet_node
            .targets
            .contains_key(&TargetPath::new("watch.pet.panel")));
        assert!(pet_node
            .targets
            .contains_key(&TargetPath::new("watch.pet.art")));

        let pet_art = layout.target(TargetPath::new("watch.pet.art")).unwrap();
        assert_eq!(pet_art.rect.width, 13);
        assert_eq!(pet_art.rect.height, 10);
        assert_eq!(pet_art.rect, Rect::new(19, 6, 13, 10));
    }

    #[test]
    fn layout_watch_compact_records_bio_hidden_decision() {
        let vm = WatchViewModel::fixture();
        let layout = layout_watch(Rect::new(0, 0, 72, 24), &vm);

        assert_eq!(layout.mode, LayoutMode::Compact);
        assert_eq!(layout.frame, Rect::new(0, 0, 72, 24));
        assert_eq!(layout.content, Rect::new(1, 1, 70, 22));
        assert_eq!(
            layout.node(WatchComponentId::Pet.path()).unwrap().bounds,
            Rect::new(1, 1, 70, 4)
        );
        assert_eq!(
            layout.node(WatchComponentId::Vitals.path()).unwrap().bounds,
            Rect::new(1, 5, 70, 4)
        );
        assert_eq!(
            layout.node(WatchComponentId::Today.path()).unwrap().bounds,
            Rect::new(1, 10, 70, 6)
        );
        assert_eq!(
            layout
                .node(WatchComponentId::Progress.path())
                .unwrap()
                .bounds,
            Rect::new(1, 17, 70, 2)
        );
        assert_eq!(
            layout.node(WatchComponentId::Feed.path()).unwrap().bounds,
            Rect::new(1, 20, 70, 3)
        );

        let bio = layout.node(WatchComponentId::Bio.path()).unwrap();
        assert!(matches!(
            bio.visibility,
            VisibilityState::Hidden {
                reason: LayoutDecisionReason::CompactMode
            }
        ));
        assert!(
            layout
                .decisions
                .iter()
                .any(|d| d.path == WatchComponentId::Bio.path()
                    && d.reason == LayoutDecisionReason::CompactMode),
            "compact bio hide decision missing"
        );
    }

    #[test]
    fn layout_watch_compact_omits_pet_targets_when_renderer_skips_pet() {
        let vm = WatchViewModel::fixture();
        let layout = layout_watch(Rect::new(0, 0, 72, 24), &vm);

        let pet = layout.node(WatchComponentId::Pet.path()).unwrap();
        assert_eq!(pet.bounds, Rect::new(1, 1, 70, 4));
        assert!(matches!(
            pet.visibility,
            VisibilityState::Degraded {
                reason: LayoutDecisionReason::InsufficientHeight
            } | VisibilityState::Hidden {
                reason: LayoutDecisionReason::InsufficientHeight
            }
        ));
        assert!(
            pet.targets.is_empty(),
            "skipped pet should export no targets"
        );
        assert!(layout.target(TargetPath::new("watch.pet.panel")).is_none());
        assert!(layout.target(TargetPath::new("watch.pet.art")).is_none());
        assert!(
            layout
                .decisions
                .iter()
                .any(|d| d.path == WatchComponentId::Pet.path()
                    && d.reason == LayoutDecisionReason::InsufficientHeight),
            "compact pet insufficient-height decision missing"
        );
    }

    #[test]
    fn layout_watch_wide_speech_active_pet_targets_stay_inside_pet_node() {
        let mut vm = WatchViewModel::fixture();
        vm.current_speech = Some("hello".into());
        let layout = layout_watch(Rect::new(0, 0, 120, 32), &vm);
        let pet = layout.node(WatchComponentId::Pet.path()).unwrap();

        for target in [
            TargetPath::new("watch.pet.art"),
            TargetPath::new("watch.pet.effect"),
            TargetPath::new("watch.pet.hit"),
            TargetPath::new("watch.pet.speech"),
        ] {
            let target = layout.target(target).unwrap();
            assert_rect_contains(pet.bounds, target.rect);
            assert_rect_contains(target.clip, target.rect);
        }
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
