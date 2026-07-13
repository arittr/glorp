pub mod color_ops;
pub mod companion_scene;
pub mod draw_list;
pub mod effect;
pub mod gauge_values;
pub mod habitat_inventory;
pub mod pet;
pub mod pet_scene;
pub mod pixel;
mod pixel_input;
pub mod privacy;
pub mod props;
pub mod rasterize;
pub mod room;
pub mod scene;
pub mod smooth;
pub mod surface;
pub(crate) mod tank_life;
pub mod target;

pub use draw_list::{DrawCell, SceneDrawList};
pub use effect::EffectState;
pub use pet_scene::PetSceneModel;
pub use rasterize::{rasterize, RasterCell};
pub use surface::{
    Clip, Detail, EyeEmphasis, LiveColorInputs, ResolvedColors, SurfaceStyle, MENU_STYLE,
    PIXEL_STYLE, ROUND_STYLE, SCREEN_STYLE, WATCH_STYLE,
};
