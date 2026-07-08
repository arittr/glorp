use clap::ValueEnum;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompanionReviewOptions {
    pub initial_size: Option<CompanionReviewSize>,
    pub active_pulse: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompanionReviewSize {
    pub width: u16,
    pub height: u16,
}

impl std::str::FromStr for CompanionReviewSize {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some((width, height)) = value.split_once('x') else {
            return Err("expected WIDTHxHEIGHT, for example 260x260".to_string());
        };
        let width = width
            .parse::<u16>()
            .map_err(|_| "width must be an integer".to_string())?;
        let height = height
            .parse::<u16>()
            .map_err(|_| "height must be an integer".to_string())?;
        if width < 120 || height < 120 {
            return Err("review size must be at least 120x120".to_string());
        }
        Ok(Self { width, height })
    }
}

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
