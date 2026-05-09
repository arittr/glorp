use std::{
    io::{self, Stdout},
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, Show},
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
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
            usage_poll_interval: Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overlay {
    Help,
    Evolution,
    Hatch,
}

pub struct WatchApp {
    vm: WatchViewModel,
    config: WatchAppConfig,
    overlay: Option<Overlay>,
    poller: Box<dyn WatchUsagePoller>,
    poll_count: u64,
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
        let mut last_poll = Instant::now();
        loop {
            terminal.draw(|frame| {
                render_watch_frame(frame, &self.vm);
                match self.overlay {
                    Some(Overlay::Help) => render_help_overlay(frame),
                    Some(Overlay::Evolution) => render_evolution_overlay(frame),
                    Some(Overlay::Hatch) => render_hatch_overlay(frame),
                    None => {}
                }
            })?;

            if event::poll(self.config.animation_tick)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Char('?') => self.overlay = Some(Overlay::Help),
                            KeyCode::Char('e') => self.overlay = Some(Overlay::Evolution),
                            KeyCode::Char('h') => self.overlay = Some(Overlay::Hatch),
                            KeyCode::Esc => self.overlay = None,
                            KeyCode::Char('r') => {
                                self.poll_usage()?;
                            }
                            KeyCode::Char('p') => self.add_affection(),
                            _ => {}
                        }
                    }
                }
            }

            if last_poll.elapsed() >= self.config.usage_poll_interval {
                self.poll_usage()?;
                last_poll = Instant::now();
            }
        }
        Ok(())
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

    fn poll_usage(&mut self) -> Result<WatchViewModel> {
        self.vm = self.poller.poll_usage(&self.vm)?;
        self.poll_count += 1;
        Ok(self.vm.clone())
    }

    fn add_affection(&mut self) {
        self.vm.happiness = (self.vm.happiness + 0.05).min(1.0);
        self.vm.recent_events.push(EventView {
            timestamp: "--:--".into(),
            kind: LogKind::Help,
            text: "a quiet pat raised happiness".into(),
        });
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
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture, Hide)?;
        Ok(Self { active: true })
    }
}

impl Drop for TerminalRestoreGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = disable_raw_mode();
            let _ = execute!(
                io::stdout(),
                Show,
                DisableMouseCapture,
                LeaveAlternateScreen
            );
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
    if value.abs() >= 1_000.0 {
        format!("{:.1}k", value / 1_000.0)
    } else {
        format!("{value:.0}")
    }
}
