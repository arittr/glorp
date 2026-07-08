use clap::ValueEnum;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum CompanionRendererMode {
    #[default]
    Classic,
    Pixel,
}

impl CompanionRendererMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            CompanionRendererMode::Classic => "classic",
            CompanionRendererMode::Pixel => "pixel",
        }
    }

    pub const fn is_pixel(self) -> bool {
        matches!(self, CompanionRendererMode::Pixel)
    }
}
