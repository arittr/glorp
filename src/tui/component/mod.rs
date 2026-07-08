pub mod geometry;
pub mod habitat_props;
pub mod ids;
pub mod pet_scene;
pub mod preview;
pub mod sizing;
pub mod style;
pub mod taffy_backend;
pub mod tank_life;
pub mod watch_screen;
pub mod widgets;

pub use geometry::{
    hit_test, ComponentLayout, ComponentNodeLayout, GeometryTarget, HitResult, LayoutDecision,
    LayoutDecisionReason, LayoutMode, TargetRole, VisibilityState,
};
pub use habitat_props::{habitat_props_for, HabitatPropCell};
pub use ids::{ComponentPath, TargetPath, WatchComponentId};
pub use pet_scene::{PetScene, PetSceneLayout};
pub use preview::{
    preview_layout, PreviewLayout, PreviewLayoutDecision, PreviewRect, PreviewTarget,
};
pub use sizing::{AxisSize, ComponentSizing, DegradeRule};
pub use style::{BorderTone, ComponentStyle, GradientToken, Insets, Surface, TextTone};
pub use tank_life::{
    anemone_anchor_sprite, anemone_morph_for_day, canonical_daily_cast, host_fish_sprite,
    layer_segment_summaries, project_tank_life_cast, rect_contains, tank_life_placements_for,
    validate_tank_life_catalog, AnemoneMorph, RenderedTankLifeCast, RoundApertureMask, SpriteCell,
    TankLifeCell, TankLifeLayerSegmentSummary, TankLifePlacement, TankLifeRenderInput,
    TankLifeSkip, TankLifeSkipReason, TankLifeSurface, TankLifeSurfaceGeometry,
};
pub use watch_screen::{layout_watch, layout_watch_with_context, render_watch_layout};
pub use widgets::{
    FeedList, InlineSparkline, MetricRow, Panel as ComponentPanel, ProgressBar, StatRow, TextRow,
};
