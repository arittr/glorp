use crate::tui::component::{
    taffy_backend::{self, ColumnSpec, RowSpec},
    ComponentLayout, ComponentNodeLayout, ComponentPath, LayoutDecision, LayoutDecisionReason,
    LayoutMode, PetScene, TargetPath, VisibilityState, WatchComponentId,
};
use crate::tui::panels::{
    BioCardPanel, FeedPanel, LegacyPanel, PetPanel, ProgressPanel, TodayPanel, VitalsPanel,
};
use crate::tui::render_context::RenderContext;
use crate::tui::style::ColorCapability;
use crate::tui::view_model::WatchViewModel;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::Block;

/// Smallest *terminal width* that uses the wide two-column layout is
/// `COMPACT_THRESHOLD + 2` (the +2 accounts for the outer rounded-border cells).
/// Below the resulting threshold of 118 cells we fall back to the single-column
/// compact layout.
pub const COMPACT_THRESHOLD: usize = 116;

/// Width of the left info column (vitals/progress/bio) in the wide layout's
/// info band. The right info column (today/feed) fills the remaining space.
pub const WIDE_INFO_LEFT_COL: u16 = 48;

/// Left column width in the wide layout (pet + vitals column).
/// Kept for any external references; the layout now uses WIDE_INFO_LEFT_COL.
pub const WIDE_LEFT_COL: u16 = 52;

/// Gutter width between left and right columns in the wide layout.
pub const WIDE_GUTTER: u16 = 4;

/// Maximum frame width. Wide mode centers this width inside oversized
/// terminals while using the terminal's full height.
pub const MAX_FRAME_WIDTH: u16 = 124;

/// Vertical padding inside the rounded frame, between the chrome border and
/// the first/last panel.
pub const INNER_VPAD: u16 = 1;

/// Gap between stacked panels in both wide and compact layouts.
pub const COLUMN_GAP: u16 = 1;

pub const RIGHT_MIN_WIDTH: u16 = 50;

/// Current renderer only draws the pet panel when the allocated area can hold
/// the 13x10 pet art.
pub const PET_RENDER_MIN_HEIGHT: u16 = 10;

pub const FEED_MAX_HEIGHT: u16 = 8;
pub const COMPACT_TODAY_MIN_HEIGHT: u16 = 4;
pub const COMPACT_FEED_MIN_HEIGHT: u16 = 2;

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
    let pet_vm;
    let pet_vm_ref = if speech_hidden_by_layout(layout) {
        pet_vm = vm_without_speech(vm);
        &pet_vm
    } else {
        vm
    };
    render_if_visible(
        layout,
        WatchComponentId::Pet.path(),
        buf,
        pet_vm_ref,
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

/// Returns the watch frame rect inside `terminal_area`.
///
/// Wide terminals cap and center the frame width, while both wide and compact
/// modes use the terminal's full available height.
pub fn bounded_frame_rect(terminal_area: Rect) -> Rect {
    let width = terminal_area.width.min(MAX_FRAME_WIDTH);
    let height = terminal_area.height;
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

    let vitals_height = intrinsic_height(VitalsPanel.preferred_constraint(vm));
    let progress_height = intrinsic_height(ProgressPanel.preferred_constraint(vm));
    let bio_height = intrinsic_height(BioCardPanel.preferred_constraint(vm));
    let today_height = intrinsic_height(TodayPanel.preferred_constraint(vm));
    let feed_height = bounded_feed_height(vm);

    let left_stack_height = vitals_height
        .saturating_add(COLUMN_GAP)
        .saturating_add(progress_height)
        .saturating_add(COLUMN_GAP)
        .saturating_add(bio_height);
    let right_stack_height = today_height
        .saturating_add(COLUMN_GAP)
        .saturating_add(feed_height);
    let info_band_height = left_stack_height.max(right_stack_height);

    let body = taffy_backend::allocate_stack(
        padded,
        &[
            RowSpec::fill(1),
            RowSpec::fixed(COLUMN_GAP),
            RowSpec::fixed(info_band_height),
        ],
    );

    let info = taffy_backend::allocate_columns(
        body[2],
        &[
            ColumnSpec::fixed(WIDE_INFO_LEFT_COL),
            ColumnSpec::fixed(WIDE_GUTTER),
            ColumnSpec::fill(1),
        ],
    );

    let left_info = taffy_backend::allocate_stack(
        info[0],
        &[
            RowSpec::fixed(vitals_height),
            RowSpec::fixed(COLUMN_GAP),
            RowSpec::fixed(progress_height),
            RowSpec::fixed(COLUMN_GAP),
            RowSpec::fixed(bio_height),
            RowSpec::fill(1),
        ],
    );

    let right_info = taffy_backend::allocate_stack(
        info[2],
        &[
            RowSpec::fixed(today_height),
            RowSpec::fixed(COLUMN_GAP),
            RowSpec::fill(1),
        ],
    );

    insert_pet_node(layout, body[0], vm, ctx);
    insert_visible_node(layout, WatchComponentId::Vitals, left_info[0]);
    insert_visible_node(layout, WatchComponentId::Progress, left_info[2]);
    insert_visible_node(layout, WatchComponentId::Bio, left_info[4]);
    insert_visible_node(layout, WatchComponentId::Today, right_info[0]);
    insert_visible_node(layout, WatchComponentId::Feed, right_info[2]);
}

fn layout_compact(
    area: Rect,
    vm: &WatchViewModel,
    ctx: &RenderContext,
    layout: &mut ComponentLayout,
) {
    let vitals_height = intrinsic_height(VitalsPanel.preferred_constraint(vm));
    let today_preferred_height = intrinsic_height(TodayPanel.preferred_constraint(vm));
    let progress_height = intrinsic_height(ProgressPanel.preferred_constraint(vm));
    let feed_preferred_height = bounded_feed_height(vm);
    let today_min_height = today_preferred_height.min(COMPACT_TODAY_MIN_HEIGHT);
    let feed_min_height = feed_preferred_height.min(COMPACT_FEED_MIN_HEIGHT);

    let mut remaining = area.height;
    let base_pet_height = PET_RENDER_MIN_HEIGHT.min(remaining);
    remaining = remaining.saturating_sub(base_pet_height);
    let vitals_height = vitals_height.min(remaining);
    remaining = remaining.saturating_sub(vitals_height);
    let mut today_height = today_min_height.min(remaining);
    remaining = remaining.saturating_sub(today_height);
    let progress_height = progress_height.min(remaining);
    remaining = remaining.saturating_sub(progress_height);
    let mut feed_height = feed_min_height.min(remaining);
    remaining = remaining.saturating_sub(feed_height);

    let gap_height = if remaining >= COLUMN_GAP * 3 {
        remaining = remaining.saturating_sub(COLUMN_GAP * 3);
        COLUMN_GAP
    } else {
        0
    };

    let today_extra = today_preferred_height
        .saturating_sub(today_height)
        .min(remaining);
    today_height = today_height.saturating_add(today_extra);
    remaining = remaining.saturating_sub(today_extra);

    let feed_extra = feed_preferred_height
        .saturating_sub(feed_height)
        .min(remaining);
    feed_height = feed_height.saturating_add(feed_extra);
    remaining = remaining.saturating_sub(feed_extra);

    let hide_speech = vm.current_speech.is_some() && remaining == 0;
    let speech_extra = if vm.current_speech.is_some() && !hide_speech && remaining > 0 {
        remaining = remaining.saturating_sub(1);
        1
    } else {
        0
    };

    let pet_height = base_pet_height
        .saturating_add(speech_extra)
        .saturating_add(remaining);

    let stack = taffy_backend::allocate_stack(
        area,
        &[
            RowSpec::fixed(pet_height),
            RowSpec::fixed(vitals_height),
            RowSpec::fixed(gap_height),
            RowSpec::fixed(today_height),
            RowSpec::fixed(gap_height),
            RowSpec::fixed(progress_height),
            RowSpec::fixed(gap_height),
            RowSpec::fixed(feed_height),
        ],
    );

    insert_hidden_bio(layout, area);
    if today_preferred_height > today_height {
        layout.decisions.push(LayoutDecision {
            path: WatchComponentId::Today.path(),
            reason: LayoutDecisionReason::RowLimit,
            message: "today rows limited by compact layout height",
        });
    }
    if feed_preferred_height > feed_height {
        layout.decisions.push(LayoutDecision {
            path: WatchComponentId::Feed.path(),
            reason: LayoutDecisionReason::RowLimit,
            message: "feed rows limited by compact layout height",
        });
    }
    if hide_speech {
        layout.decisions.push(LayoutDecision {
            path: ComponentPath::new("watch.pet.speech"),
            reason: LayoutDecisionReason::InsufficientHeight,
            message: "pet speech hidden to preserve pet art in compact layout",
        });
    }

    insert_pet_node_with_speech_policy(layout, stack[0], vm, ctx, hide_speech);
    insert_visible_node(layout, WatchComponentId::Vitals, stack[1]);
    insert_visible_node(layout, WatchComponentId::Today, stack[3]);
    insert_visible_node(layout, WatchComponentId::Progress, stack[5]);
    insert_feed_node(layout, stack[7]);
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

fn insert_feed_node(layout: &mut ComponentLayout, bounds: Rect) {
    let mut node = ComponentNodeLayout::leaf(WatchComponentId::Feed.path(), bounds);
    if bounds.height == 0 {
        node.visibility = VisibilityState::Hidden {
            reason: LayoutDecisionReason::InsufficientHeight,
        };
        layout.decisions.push(LayoutDecision {
            path: WatchComponentId::Feed.path(),
            reason: LayoutDecisionReason::InsufficientHeight,
            message: "feed hidden because compact layout has no drawable rows left",
        });
    }
    layout
        .insert_node(node)
        .expect("feed component id is unique in watch layout");
}

fn insert_pet_node(
    layout: &mut ComponentLayout,
    bounds: Rect,
    vm: &WatchViewModel,
    ctx: &RenderContext,
) {
    insert_pet_node_with_speech_policy(layout, bounds, vm, ctx, false);
}

fn insert_pet_node_with_speech_policy(
    layout: &mut ComponentLayout,
    bounds: Rect,
    vm: &WatchViewModel,
    ctx: &RenderContext,
    hide_speech: bool,
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

    let speechless_vm;
    let scene_vm = if hide_speech {
        speechless_vm = vm_without_speech(vm);
        &speechless_vm
    } else {
        vm
    };
    let scene = PetScene::compute_layout(bounds, scene_vm, ctx);
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

fn bounded_feed_height(vm: &WatchViewModel) -> u16 {
    intrinsic_height(FeedPanel.preferred_constraint(vm)).min(FEED_MAX_HEIGHT)
}

fn intrinsic_height(constraint: Constraint) -> u16 {
    match constraint {
        Constraint::Length(height)
        | Constraint::Min(height)
        | Constraint::Max(height)
        | Constraint::Percentage(height)
        | Constraint::Fill(height) => height,
        Constraint::Ratio(numerator, denominator) => numerator
            .checked_div(denominator)
            .unwrap_or(0)
            .min(u16::MAX as u32) as u16,
    }
}

fn speech_hidden_by_layout(layout: &ComponentLayout) -> bool {
    layout.decisions.iter().any(|decision| {
        decision.path == ComponentPath::new("watch.pet.speech")
            && decision.reason == LayoutDecisionReason::InsufficientHeight
    })
}

fn vm_without_speech(vm: &WatchViewModel) -> WatchViewModel {
    let mut vm = vm.clone();
    vm.current_speech = None;
    vm
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
        assert_eq!(layout.frame, Rect::new(0, 0, 120, 32));
        assert_eq!(layout.content, Rect::new(1, 1, 118, 30));
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

        // Pet spans full body width at the top.
        // fixture feed_height=3 (2 events + 1 header), so:
        //   left_stack=4+1+2+1+3=11, right_stack=6+1+3=10, info_band=11
        //   padded height=28, pet = 28-1-11=16
        assert_eq!(
            layout.node(WatchComponentId::Pet.path()).unwrap().bounds,
            Rect::new(1, 2, 118, 16)
        );
        // Left info column: x=1, w=48. Info band starts at y=19 (2+16+1).
        assert_eq!(
            layout.node(WatchComponentId::Vitals.path()).unwrap().bounds,
            Rect::new(1, 19, 48, 4)
        );
        assert_eq!(
            layout
                .node(WatchComponentId::Progress.path())
                .unwrap()
                .bounds,
            Rect::new(1, 24, 48, 2)
        );
        assert_eq!(
            layout.node(WatchComponentId::Bio.path()).unwrap().bounds,
            Rect::new(1, 27, 48, 3)
        );
        // Right info column: x=53 (1+48+4), w=66 (118-48-4).
        assert_eq!(
            layout.node(WatchComponentId::Today.path()).unwrap().bounds,
            Rect::new(53, 19, 66, 6)
        );
        // Feed fills right col remainder: 11-6-1=4 rows.
        assert_eq!(
            layout.node(WatchComponentId::Feed.path()).unwrap().bounds,
            Rect::new(53, 26, 66, 4)
        );

        let pet_panel = layout.target(TargetPath::new("watch.pet.panel")).unwrap();
        assert_eq!(pet_panel.rect, Rect::new(1, 2, 118, 16));

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
        // Art centered in 118×16 panel: cx=1+(118-13)/2=53, cy=2+(16-10)/2=5.
        assert_eq!(pet_art.rect, Rect::new(53, 5, 13, 10));
    }

    #[test]
    fn layout_watch_wide_info_columns_share_band_top_and_bottom() {
        let vm = WatchViewModel::fixture();
        let layout = layout_watch(Rect::new(0, 0, 180, 50), &vm);
        let vitals = layout.node(WatchComponentId::Vitals.path()).unwrap().bounds;
        let today = layout.node(WatchComponentId::Today.path()).unwrap().bounds;
        let bio = layout.node(WatchComponentId::Bio.path()).unwrap().bounds;
        let feed = layout.node(WatchComponentId::Feed.path()).unwrap().bounds;

        // Both columns start at the same y (top of info band).
        assert_eq!(
            vitals.y, today.y,
            "left and right info columns should start at the same y"
        );
        // Feed fills the right column below today; feed is the last right-column
        // item and absorbs any slack, so right column bottom = today + gap + feed.
        assert!(
            feed.y > today.y,
            "feed should be below today in the right column"
        );
        // Left column: bio is the last pinned item; right column: feed is the last item.
        // Both column bottoms should be within the same info band.
        let left_col_bottom = bio.y.saturating_add(bio.height);
        let right_col_bottom = feed.y.saturating_add(feed.height);
        assert_eq!(
            left_col_bottom, right_col_bottom,
            "left and right info column bottoms should align (same info band height)"
        );
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
            Rect::new(1, 1, 70, 10)
        );
        assert_eq!(
            layout.node(WatchComponentId::Vitals.path()).unwrap().bounds,
            Rect::new(1, 11, 70, 4)
        );
        assert_eq!(
            layout.node(WatchComponentId::Today.path()).unwrap().bounds,
            Rect::new(1, 15, 70, 4)
        );
        assert_eq!(
            layout
                .node(WatchComponentId::Progress.path())
                .unwrap()
                .bounds,
            Rect::new(1, 19, 70, 2)
        );
        assert_eq!(
            layout.node(WatchComponentId::Feed.path()).unwrap().bounds,
            Rect::new(1, 21, 70, 2)
        );

        let bio = layout.node(WatchComponentId::Bio.path()).unwrap();
        assert!(matches!(
            bio.visibility,
            VisibilityState::Hidden {
                reason: LayoutDecisionReason::CompactMode
            }
        ));
        assert!(
            matches!(
                layout.decisions.as_slice(),
                [
                    LayoutDecision {
                        path,
                        reason: LayoutDecisionReason::CompactMode,
                        ..
                    },
                    LayoutDecision {
                        path: today_path,
                        reason: LayoutDecisionReason::RowLimit,
                        ..
                    },
                    LayoutDecision {
                        path: feed_path,
                        reason: LayoutDecisionReason::RowLimit,
                        ..
                    },
                    ..
                ] if *path == WatchComponentId::Bio.path()
                    && *today_path == WatchComponentId::Today.path()
                    && *feed_path == WatchComponentId::Feed.path()
            ),
            "compact bio hide decision missing"
        );
    }

    #[test]
    fn layout_watch_compact_preserves_pet_targets_at_default_height() {
        let vm = WatchViewModel::fixture();
        let layout = layout_watch(Rect::new(0, 0, 72, 24), &vm);

        let pet = layout.node(WatchComponentId::Pet.path()).unwrap();
        assert_eq!(pet.bounds, Rect::new(1, 1, 70, 10));
        assert_eq!(pet.visibility, VisibilityState::Visible);
        assert!(layout.target(TargetPath::new("watch.pet.panel")).is_some());
        assert!(layout.target(TargetPath::new("watch.pet.art")).is_some());
        assert!(
            layout
                .decisions
                .iter()
                .any(|d| d.path == WatchComponentId::Feed.path()
                    && d.reason == LayoutDecisionReason::RowLimit),
            "compact feed row-limit decision missing"
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

    #[test]
    fn wide_layout_pet_panel_spans_body_width() {
        let vm = WatchViewModel::fixture();
        let layout = layout_watch(Rect::new(0, 0, 120, 32), &vm);
        let pet = layout.node(WatchComponentId::Pet.path()).expect("pet node");
        assert_eq!(pet.bounds.x, 1, "pet should start at body left edge");
        assert!(
            pet.bounds.width >= 110,
            "pet should span ~full body width; got width={}",
            pet.bounds.width
        );
    }

    #[test]
    fn wide_layout_info_band_is_two_columns_below_pet() {
        let vm = WatchViewModel::fixture();
        let layout = layout_watch(Rect::new(0, 0, 120, 32), &vm);
        let vitals = layout
            .node(WatchComponentId::Vitals.path())
            .expect("vitals");
        let today = layout.node(WatchComponentId::Today.path()).expect("today");
        let pet = layout.node(WatchComponentId::Pet.path()).expect("pet");

        // Vitals on left, today on right, both below pet.
        assert_eq!(vitals.bounds.x, 1, "vitals on left edge");
        assert!(
            today.bounds.x > vitals.bounds.x + vitals.bounds.width,
            "today to the right of vitals with gutter"
        );
        assert!(
            vitals.bounds.y >= pet.bounds.y + pet.bounds.height,
            "vitals below pet"
        );
    }

    #[test]
    fn compact_threshold_boundary() {
        use crate::tui::component::LayoutMode;
        let vm = crate::tui::view_model::WatchViewModel::fixture();

        let just_below = layout_watch(Rect::new(0, 0, 117, 30), &vm);
        assert_eq!(
            just_below.mode,
            LayoutMode::Compact,
            "117 cols should be compact"
        );

        let at_threshold = layout_watch(Rect::new(0, 0, 118, 30), &vm);
        assert_eq!(
            at_threshold.mode,
            LayoutMode::Wide,
            "118 cols should be wide"
        );
    }
}
