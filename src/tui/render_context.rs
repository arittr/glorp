use crate::tui::style::ColorCapability;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderContext {
    pub color_capability: ColorCapability,
}

impl RenderContext {
    pub const fn new(color_capability: ColorCapability) -> Self {
        Self { color_capability }
    }

    pub fn from_environment() -> Self {
        Self::new(ColorCapability::detect())
    }
}

impl Default for RenderContext {
    fn default() -> Self {
        Self::from_environment()
    }
}
