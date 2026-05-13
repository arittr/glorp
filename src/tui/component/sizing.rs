use crate::tui::component::ids::TargetPath;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentSizing {
    pub width: AxisSize,
    pub height: AxisSize,
    pub degrade: Vec<DegradeRule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisSize {
    Fixed(u16),
    Intrinsic {
        min: u16,
        preferred: u16,
        max: Option<u16>,
    },
    Fill {
        min: u16,
        weight: u16,
        max: Option<u16>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradeRule {
    LimitRows {
        target: TargetPath,
        min: u16,
        max: u16,
    },
    OmitDetail {
        target: TargetPath,
    },
    HideTarget {
        target: TargetPath,
    },
    HideComponent,
}
