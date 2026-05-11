use crate::tui::style::LogKind;

#[derive(Debug, Clone, PartialEq)]
pub struct WatchViewModel {
    pub pet_art: Vec<String>,
    pub pet_spans: Vec<crate::pet::render::StyledSegment>,
    pub pet_render: PetRenderModel,
    pub pet_name: String,
    pub species: String,
    pub stage: String,
    pub mood: String,
    pub age_days: u32,
    pub xp_current: f64,
    pub xp_target: f64,
    pub fed: f64,
    pub happiness: f64,
    pub energy: f64,
    pub today_effective_tokens: f64,
    pub recent_daily_effective_tokens: Vec<f64>,
    pub source_breakdown: Vec<SourceUsageView>,
    pub source_health: Vec<SourceHealthView>,
    pub current_bucket_effective_tokens: f64,
    pub recent_events: Vec<EventView>,
    pub helper_status: String,
    pub errors: Vec<String>,
    pub latest_evolution: Option<String>,
    pub acknowledged_evolution: Option<String>,
    /// Raw mouse position in screen coordinates, or None when the cursor is
    /// off the terminal or mouse tracking is disabled. PetPanel hit-tests
    /// against its rect to decide whether to swap to cursor-tracked eyes.
    pub cursor_screen: Option<(u16, u16)>,
    /// Whether mouse tracking is currently enabled (toggled via `m` key).
    pub mouse_tracking_enabled: bool,
    /// Pet's current speech-bubble line, or None when the bubble is hidden.
    /// Computed deterministically from mood + recent token activity + wall
    /// clock; cycles ~5s visible / ~25s hidden.
    pub current_speech: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PetRenderModel {
    pub seed: String,
    pub generated_species: String,
    pub stage: String,
    pub mood: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceUsageView {
    pub name: String,
    pub effective_tokens: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceStatus {
    Ready,
    Diagnostic,
    Blocked,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SourceHealthView {
    pub name: String,
    pub status: SourceStatus,
    pub today_effective_tokens: f64,
    pub bucket_effective_tokens: f64,
    pub diagnostic_code: Option<String>,
    pub diagnostic_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EventView {
    pub timestamp: String,
    pub kind: LogKind,
    pub text: String,
}

impl WatchViewModel {
    pub fn fixture() -> Self {
        Self {
            pet_art: vec!["  /\\_/\\  ".into(), " ( o.o ) ".into(), "  > ^ <  ".into()],
            pet_spans: Vec::new(),
            pet_render: PetRenderModel {
                seed: "fixture-seed".into(),
                generated_species: "fuzz".into(),
                stage: "s0".into(),
                mood: "content".into(),
            },
            pet_name: "miso".into(),
            species: "terminal sprout".into(),
            stage: "hatchling".into(),
            mood: "curious".into(),
            age_days: 4,
            xp_current: 42_000.0,
            xp_target: 100_000.0,
            fed: 0.72,
            happiness: 0.64,
            energy: 0.81,
            today_effective_tokens: 18_420.0,
            recent_daily_effective_tokens: vec![
                1_000.0, 8_000.0, 4_000.0, 13_000.0, 9_500.0, 16_000.0, 18_420.0,
            ],
            source_breakdown: vec![
                SourceUsageView {
                    name: "claude-code".into(),
                    effective_tokens: 12_900.0,
                },
                SourceUsageView {
                    name: "codex".into(),
                    effective_tokens: 5_520.0,
                },
            ],
            source_health: vec![
                SourceHealthView {
                    name: "claude-code".into(),
                    status: SourceStatus::Ready,
                    today_effective_tokens: 12_900.0,
                    bucket_effective_tokens: 1_300.0,
                    diagnostic_code: None,
                    diagnostic_message: None,
                },
                SourceHealthView {
                    name: "codex".into(),
                    status: SourceStatus::Ready,
                    today_effective_tokens: 5_520.0,
                    bucket_effective_tokens: 1_000.0,
                    diagnostic_code: None,
                    diagnostic_message: None,
                },
            ],
            current_bucket_effective_tokens: 2_300.0,
            recent_events: vec![
                EventView {
                    timestamp: "13:38".into(),
                    kind: LogKind::Normal,
                    text: "watch loop settled; next poll in 60s".into(),
                },
                EventView {
                    timestamp: "13:40".into(),
                    kind: LogKind::Usage,
                    text: "fed from 2.3k effective tokens".into(),
                },
            ],
            helper_status: "helper ready".into(),
            errors: Vec::new(),
            latest_evolution: None,
            acknowledged_evolution: None,
            cursor_screen: None,
            mouse_tracking_enabled: true,
            current_speech: None,
        }
    }

    pub fn fixture_with_events() -> Self {
        let mut vm = Self::fixture();
        vm.recent_events = vec![
            EventView {
                timestamp: "13:42".into(),
                kind: LogKind::Usage,
                text: "claude-code added 1.3k effective tokens".into(),
            },
            EventView {
                timestamp: "13:42".into(),
                kind: LogKind::Diagnostic,
                text: "ccusage helper returned a retryable diagnostic".into(),
            },
            EventView {
                timestamp: "13:42".into(),
                kind: LogKind::Evolution,
                text: "miso shimmered toward sproutling".into(),
            },
        ];
        vm.latest_evolution = Some("sproutling shimmer ready".into());
        vm
    }

    pub fn is_blocked(&self) -> bool {
        !self.source_health.is_empty()
            && self.source_health.iter().all(|source| {
                matches!(
                    source.status,
                    SourceStatus::Blocked | SourceStatus::Diagnostic
                )
            })
    }

    pub fn should_render_evolution_moment(&self) -> bool {
        self.latest_evolution.is_some() && self.latest_evolution != self.acknowledged_evolution
    }

    pub fn acknowledge_latest_evolution(&mut self) {
        self.acknowledged_evolution = self.latest_evolution.clone();
    }

    #[doc(hidden)]
    pub fn acknowledge_latest_evolution_for_test(&mut self) {
        self.acknowledge_latest_evolution();
    }

    #[doc(hidden)]
    pub fn acknowledged_evolution_for_test(&self, transition: &str) -> bool {
        self.acknowledged_evolution.as_deref() == Some(transition)
    }
}
