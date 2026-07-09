use clap::ValueEnum;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompanionReviewOptions {
    pub initial_size: Option<CompanionReviewSize>,
    pub active_pulse: bool,
    pub state: Option<CompanionReviewState>,
    pub duration_ms: Option<u64>,
    pub capture_dir: Option<PathBuf>,
}

impl CompanionReviewOptions {
    pub const fn resolved_state(&self) -> CompanionReviewState {
        match self.state {
            Some(state) => state,
            None if self.active_pulse => CompanionReviewState::ActivePulse,
            None => CompanionReviewState::Normal,
        }
    }

    pub fn has_review_launch_options(&self) -> bool {
        self.initial_size.is_some()
            || self.active_pulse
            || self.state.is_some()
            || self.duration_ms.is_some()
            || self.capture_dir.is_some()
    }
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum CompanionReviewState {
    #[default]
    Normal,
    ActivePulse,
    AsleepCalm,
    HelperTrouble,
}

impl CompanionReviewState {
    pub const fn as_str(self) -> &'static str {
        match self {
            CompanionReviewState::Normal => "normal",
            CompanionReviewState::ActivePulse => "active-pulse",
            CompanionReviewState::AsleepCalm => "asleep-calm",
            CompanionReviewState::HelperTrouble => "helper-trouble",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CompanionReviewOptions, CompanionReviewSize, CompanionReviewState};
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

    #[test]
    fn legacy_active_pulse_maps_to_active_pulse_when_state_is_absent() {
        let review = CompanionReviewOptions {
            active_pulse: true,
            ..CompanionReviewOptions::default()
        };

        assert_eq!(review.resolved_state(), CompanionReviewState::ActivePulse);
    }

    #[test]
    fn explicit_review_state_takes_precedence_over_legacy_active_pulse() {
        let review = CompanionReviewOptions {
            active_pulse: true,
            state: Some(CompanionReviewState::AsleepCalm),
            ..CompanionReviewOptions::default()
        };

        assert_eq!(review.resolved_state(), CompanionReviewState::AsleepCalm);
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
