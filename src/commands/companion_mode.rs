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
        if width < 260 || height < 260 {
            return Err("review size must be at least 260x260".to_string());
        }
        Ok(Self { width, height })
    }
}

#[cfg(test)]
mod tests {
    use super::CompanionReviewSize;
    use std::str::FromStr;

    #[test]
    fn review_size_rejects_malformed_values() {
        assert_eq!(
            CompanionReviewSize::from_str("260").unwrap_err(),
            "expected WIDTHxHEIGHT, for example 260x260"
        );
        assert_eq!(
            CompanionReviewSize::from_str("wide x tall").unwrap_err(),
            "width must be an integer"
        );
    }

    #[test]
    fn review_size_rejects_dimensions_below_window_minimum() {
        assert_eq!(
            CompanionReviewSize::from_str("120x120").unwrap_err(),
            "review size must be at least 260x260"
        );
        assert_eq!(
            CompanionReviewSize::from_str("259x400").unwrap_err(),
            "review size must be at least 260x260"
        );
        assert_eq!(
            CompanionReviewSize::from_str("400x259").unwrap_err(),
            "review size must be at least 260x260"
        );
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum CompanionRendererMode {
    #[default]
    Classic,
    Pixel,
    Smooth,
}

impl CompanionRendererMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            CompanionRendererMode::Classic => "classic",
            CompanionRendererMode::Pixel => "pixel",
            CompanionRendererMode::Smooth => "smooth",
        }
    }

    pub const fn is_pixel(self) -> bool {
        matches!(self, CompanionRendererMode::Pixel)
    }

    pub const fn is_smooth(self) -> bool {
        matches!(self, CompanionRendererMode::Smooth)
    }
}
