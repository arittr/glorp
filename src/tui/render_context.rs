use crate::tui::style::ColorCapability;
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchClock {
    Live,
    Fixed(OffsetDateTime),
}

impl WatchClock {
    pub const fn live() -> Self {
        Self::Live
    }

    pub const fn fixed(now_utc: OffsetDateTime) -> Self {
        Self::Fixed(now_utc)
    }

    pub fn now_utc(self) -> OffsetDateTime {
        match self {
            Self::Live => OffsetDateTime::now_utc(),
            Self::Fixed(now_utc) => now_utc,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderContext {
    pub color_capability: ColorCapability,
    pub clock: WatchClock,
}

impl RenderContext {
    pub const fn new(color_capability: ColorCapability) -> Self {
        Self::with_clock(color_capability, WatchClock::live())
    }

    pub const fn with_clock(color_capability: ColorCapability, clock: WatchClock) -> Self {
        Self { color_capability, clock }
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
