use std::{
    io::{self, Stdout},
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{
    error::Result,
    tui::{
        layout::{
            render_evolution_overlay, render_hatch_overlay, render_help_overlay, render_watch_frame,
        },
        style::LogKind,
        view_model::{EventView, SourceUsageView},
    },
};

pub use crate::tui::view_model::WatchViewModel;

#[derive(Debug, Clone, Copy)]
pub struct WatchAppConfig {
    pub animation_tick: Duration,
    pub usage_poll_interval: Duration,
}

impl Default for WatchAppConfig {
    fn default() -> Self {
        Self {
            animation_tick: Duration::from_millis(250),
            usage_poll_interval: Duration::from_secs(10),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overlay {
    Help,
}

pub struct WatchApp {
    vm: WatchViewModel,
    config: WatchAppConfig,
    overlay: Option<Overlay>,
    poller: Box<dyn WatchUsagePoller>,
    poll_count: u64,
    animation_frame: u64,
    last_poll: Option<Instant>,
}

impl WatchApp {
    pub fn new(vm: WatchViewModel) -> Self {
        Self::with_config(vm, WatchAppConfig::default())
    }

    pub fn with_config(vm: WatchViewModel, config: WatchAppConfig) -> Self {
        Self::with_poll_callback(vm, config, Box::new(NoopWatchPoller))
    }

    pub fn with_poll_callback(
        vm: WatchViewModel,
        config: WatchAppConfig,
        poller: Box<dyn WatchUsagePoller>,
    ) -> Self {
        Self {
            vm,
            config,
            overlay: None,
            poller,
            poll_count: 0,
            animation_frame: 0,
            last_poll: None,
        }
    }

    pub fn run(mut self) -> Result<()> {
        let _restore = TerminalRestoreGuard::activate()?;
        let backend = CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;
        self.run_on_terminal(&mut terminal)
    }

    pub fn run_on_terminal(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<()> {
        if self.last_poll.is_none() {
            self.last_poll = Some(Instant::now());
        }
        loop {
            self.advance_animation_frame();

            let render_evolution = self.vm.should_render_evolution_moment();
            terminal.draw(|frame| {
                render_watch_frame(frame, &self.vm);
                match self.overlay {
                    Some(Overlay::Help) => render_help_overlay(frame),
                    None => {}
                }
                if render_evolution {
                    render_evolution_overlay(frame);
                }
            })?;
            if render_evolution {
                self.vm.acknowledge_latest_evolution();
            }

            if event::poll(self.config.animation_tick)? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_key(key)? {
                        break;
                    }
                }
            }

            if self
                .last_poll
                .map(|instant| instant.elapsed() >= self.config.usage_poll_interval)
                .unwrap_or(true)
            {
                self.poll_usage()?;
                self.last_poll = Some(Instant::now());
            }
        }
        Ok(())
    }

    fn advance_animation_frame(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1);
        let _ =
            crate::commands::watch::rerender_pet_for_view_model(&mut self.vm, self.animation_frame);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if key.kind == KeyEventKind::Release {
            return Ok(false);
        }

        match key.code {
            KeyCode::Char('q') => Ok(true),
            KeyCode::Char('?') => {
                self.overlay = match self.overlay {
                    Some(Overlay::Help) => None,
                    None => Some(Overlay::Help),
                };
                Ok(false)
            }
            KeyCode::Esc => {
                self.overlay = None;
                Ok(false)
            }
            KeyCode::Char('r') => {
                self.poll_usage()?;
                self.last_poll = Some(Instant::now());
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    pub fn help_visible_for_test(&self) -> bool {
        self.overlay == Some(Overlay::Help)
    }

    pub fn refresh_for_test(&mut self) -> Result<WatchViewModel> {
        self.poll_usage()
    }

    pub fn interval_poll_for_test(&mut self) -> Result<WatchViewModel> {
        self.poll_usage()
    }

    pub fn poll_count_for_test(&self) -> u64 {
        self.poll_count
    }

    pub fn interval_due_for_test(&self, elapsed_since_last_poll: Duration) -> bool {
        elapsed_since_last_poll >= self.config.usage_poll_interval
    }

    pub fn advance_animation_for_test(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1);
        let _ =
            crate::commands::watch::rerender_pet_for_view_model(&mut self.vm, self.animation_frame);
    }

    pub fn view_model_for_test(&self) -> &WatchViewModel {
        &self.vm
    }

    #[doc(hidden)]
    pub fn handle_key_for_test(&mut self, code: KeyCode, kind: KeyEventKind) -> Result<bool> {
        self.handle_key(KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind))
    }

    fn poll_usage(&mut self) -> Result<WatchViewModel> {
        self.vm = self.poller.poll_usage(&self.vm)?;
        self.poll_count += 1;
        Ok(self.vm.clone())
    }
}

pub trait WatchUsagePoller {
    fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchViewModel>;
}

struct NoopWatchPoller;

impl WatchUsagePoller for NoopWatchPoller {
    fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchViewModel> {
        Ok(current.clone())
    }
}

pub struct TerminalRestoreGuard {
    active: bool,
}

impl TerminalRestoreGuard {
    pub fn activate() -> Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(Self { active: true })
    }
}

impl Drop for TerminalRestoreGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
            self.active = false;
        }
    }
}

#[derive(Debug, Clone)]
pub struct WatchTestHarness {
    source_name: String,
    timestamp: String,
    effective_delta: f64,
    vm: WatchViewModel,
}

impl WatchTestHarness {
    pub fn with_usage_delta(source_name: &str, timestamp: &str, effective_delta: f64) -> Self {
        Self {
            source_name: source_name.into(),
            timestamp: timestamp.into(),
            effective_delta,
            vm: WatchViewModel::fixture(),
        }
    }
}

impl WatchUsagePoller for WatchTestHarness {
    fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchViewModel> {
        self.vm = current.clone();
        run_single_watch_tick_for_test(self)
    }
}

pub fn run_single_watch_tick_for_test(harness: &mut WatchTestHarness) -> Result<WatchViewModel> {
    let mut vm = harness.vm.clone();
    vm.current_bucket_effective_tokens = harness.effective_delta;
    vm.today_effective_tokens += harness.effective_delta;

    if let Some(source) = vm
        .source_breakdown
        .iter_mut()
        .find(|source| source.name == harness.source_name)
    {
        source.effective_tokens += harness.effective_delta;
    } else {
        vm.source_breakdown.push(SourceUsageView {
            name: harness.source_name.clone(),
            effective_tokens: harness.effective_delta,
        });
    }

    vm.recent_events.push(EventView {
        timestamp: timestamp_column(&harness.timestamp),
        kind: LogKind::Usage,
        text: format!(
            "{} added {} effective tokens",
            harness.source_name,
            format_tokens(harness.effective_delta)
        ),
    });
    harness.vm = vm.clone();
    Ok(vm)
}

pub fn render_frame_for_test(frame: &mut ratatui::Frame<'_>, vm: &WatchViewModel) {
    render_watch_frame(frame, vm);
}

pub fn render_help_overlay_for_test(frame: &mut ratatui::Frame<'_>) {
    render_watch_frame(frame, &WatchViewModel::fixture());
    render_help_overlay(frame);
}

pub fn render_evolution_overlay_for_test(frame: &mut ratatui::Frame<'_>) {
    render_watch_frame(frame, &WatchViewModel::fixture());
    render_evolution_overlay(frame);
}

pub fn render_hatch_overlay_for_test(frame: &mut ratatui::Frame<'_>) {
    render_watch_frame(frame, &WatchViewModel::fixture());
    render_hatch_overlay(frame);
}

fn timestamp_column(timestamp: &str) -> String {
    timestamp
        .split('T')
        .nth(1)
        .and_then(|time| time.get(0..5))
        .unwrap_or("--:--")
        .to_string()
}

fn format_tokens(value: f64) -> String {
    let value = value.max(0.0);
    if value.abs() >= 1_000.0 {
        format!("{:.1}k", value / 1_000.0)
    } else {
        format!("{value:.0}")
    }
}
