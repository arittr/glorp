use std::{
    io::{self, Stdout},
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, Show},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::{
    error::{GlorpError, Result},
    tui::{
        layout::{
            render_evolution_overlay, render_hatch_overlay, render_help_overlay,
            render_watch_frame_with_capability,
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
    pub color_capability: crate::tui::style::ColorCapability,
}

impl Default for WatchAppConfig {
    fn default() -> Self {
        Self {
            animation_tick: Duration::from_millis(250),
            usage_poll_interval: Duration::from_secs(10),
            color_capability: crate::tui::style::ColorCapability::detect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overlay {
    Help,
}

enum PollRequest {
    Poll(Box<WatchViewModel>),
    Shutdown,
}

const EVOLUTION_OVERLAY_HOLD: Duration = Duration::from_secs(3);

pub struct WatchApp {
    vm: WatchViewModel,
    config: WatchAppConfig,
    overlay: Option<Overlay>,
    request_tx: Sender<PollRequest>,
    result_rx: Receiver<Result<WatchViewModel>>,
    worker: Option<JoinHandle<()>>,
    in_flight: bool,
    poll_count: u64,
    animation_frame: u64,
    last_poll: Option<Instant>,
    last_acknowledged_evolution: Option<String>,
    evolution_overlay_started_at: Option<Instant>,
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
        mut poller: Box<dyn WatchUsagePoller>,
    ) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<PollRequest>();
        let (result_tx, result_rx) = mpsc::channel::<Result<WatchViewModel>>();
        let worker = thread::spawn(move || {
            while let Ok(request) = request_rx.recv() {
                match request {
                    PollRequest::Poll(current) => {
                        let result = poller.poll_usage(current.as_ref());
                        // If the receiver has been dropped, the app is shutting
                        // down and we just exit; nothing else cares.
                        if result_tx.send(result).is_err() {
                            break;
                        }
                    }
                    PollRequest::Shutdown => break,
                }
            }
        });
        Self {
            vm,
            config,
            overlay: None,
            request_tx,
            result_rx,
            worker: Some(worker),
            in_flight: false,
            poll_count: 0,
            animation_frame: 0,
            last_poll: None,
            last_acknowledged_evolution: None,
            evolution_overlay_started_at: None,
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

            // Drain any completed poll result before drawing so the new vm is
            // visible this frame.
            self.try_collect_poll_result()?;

            let render_evolution = self.update_evolution_overlay();
            let stage_label = self.vm.stage.clone();
            terminal.draw(|frame| {
                render_watch_frame_with_capability(frame, &self.vm, self.config.color_capability);
                match self.overlay {
                    Some(Overlay::Help) => render_help_overlay(frame),
                    None => {}
                }
                if render_evolution {
                    render_evolution_overlay(frame, Some(stage_label.as_str()));
                }
            })?;

            if event::poll(self.config.animation_tick)? {
                match event::read()? {
                    Event::Key(key) if self.handle_key(key)? => break,
                    Event::Mouse(mouse) => self.handle_mouse(mouse),
                    _ => {}
                }
            }

            if self
                .last_poll
                .map(|instant| instant.elapsed() >= self.config.usage_poll_interval)
                .unwrap_or(true)
            {
                self.kick_off_poll()?;
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

    /// Returns whether the evolution overlay should render this frame.
    /// Tracks acknowledgement on the WatchApp (not the vm) so that worker
    /// poll completions which replace `self.vm` wholesale don't reset the
    /// acknowledged-evolution state and re-fire the overlay every poll.
    /// Holds the overlay visible for `EVOLUTION_OVERLAY_HOLD` so users can
    /// actually read it.
    fn update_evolution_overlay(&mut self) -> bool {
        let pending = match &self.vm.latest_evolution {
            Some(evo) if Some(evo) != self.last_acknowledged_evolution.as_ref() => Some(evo),
            _ => None,
        };
        match (pending, self.evolution_overlay_started_at) {
            (Some(_), None) => {
                self.evolution_overlay_started_at = Some(Instant::now());
                true
            }
            (Some(_), Some(start)) if start.elapsed() < EVOLUTION_OVERLAY_HOLD => true,
            (Some(_), Some(_)) => {
                self.last_acknowledged_evolution = self.vm.latest_evolution.clone();
                self.evolution_overlay_started_at = None;
                false
            }
            (None, _) => {
                self.evolution_overlay_started_at = None;
                false
            }
        }
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
            KeyCode::Char('m') => {
                self.vm.mouse_tracking_enabled = !self.vm.mouse_tracking_enabled;
                if !self.vm.mouse_tracking_enabled {
                    self.vm.cursor_screen = None;
                }
                Ok(false)
            }
            KeyCode::Char('r') => {
                self.kick_off_poll()?;
                self.last_poll = Some(Instant::now());
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    /// Update vm.cursor_screen from a crossterm MouseEvent so PetPanel can
    /// swap to cursor-tracked eyes on its next render. Drag/scroll/release
    /// events all update position; explicit Up events with no recorded
    /// position would be impossible from crossterm so we just take whatever
    /// coordinates the event carries.
    fn handle_mouse(&mut self, mouse: MouseEvent) {
        if !self.vm.mouse_tracking_enabled {
            return;
        }
        match mouse.kind {
            MouseEventKind::Moved
            | MouseEventKind::Drag(_)
            | MouseEventKind::Down(_)
            | MouseEventKind::Up(_) => {
                self.vm.cursor_screen = Some((mouse.column, mouse.row));
            }
            _ => {}
        }
    }

    /// Send a poll request to the worker if one is not already in flight.
    /// Polls are deduped: while a poll is outstanding, additional kickoffs
    /// are silently ignored. The interval timer keeps ticking; the next
    /// eligible poll happens after the current one completes.
    fn kick_off_poll(&mut self) -> Result<()> {
        if self.in_flight {
            return Ok(());
        }
        self.request_tx
            .send(PollRequest::Poll(Box::new(self.vm.clone())))
            .map_err(|err| GlorpError::Message(format!("watch poll worker hung up: {err}")))?;
        self.in_flight = true;
        Ok(())
    }

    /// Non-blocking collection of any completed poll result.
    fn try_collect_poll_result(&mut self) -> Result<()> {
        match self.result_rx.try_recv() {
            Ok(result) => {
                self.vm = result?;
                self.poll_count += 1;
                self.in_flight = false;
                Ok(())
            }
            Err(TryRecvError::Empty) => Ok(()),
            Err(TryRecvError::Disconnected) => {
                Err(GlorpError::Message("watch poll worker disconnected".into()))
            }
        }
    }

    pub fn help_visible_for_test(&self) -> bool {
        self.overlay == Some(Overlay::Help)
    }

    pub fn refresh_for_test(&mut self) -> Result<WatchViewModel> {
        self.kick_off_poll()?;
        self.await_pending_poll_for_test()?;
        Ok(self.vm.clone())
    }

    pub fn interval_poll_for_test(&mut self) -> Result<WatchViewModel> {
        self.refresh_for_test()
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

    pub fn in_flight_for_test(&self) -> bool {
        self.in_flight
    }

    /// Send a poll request without blocking. Returns whether a request was
    /// actually sent (false if a poll was already in flight). Mirrors the
    /// production interval/refresh kickoff path.
    pub fn kick_off_poll_for_test(&mut self) -> Result<bool> {
        let was_in_flight = self.in_flight;
        self.kick_off_poll()?;
        Ok(!was_in_flight)
    }

    /// Block until any in-flight poll result lands. Tests rely on this to
    /// drive the worker synchronously even though production keeps the
    /// main loop non-blocking.
    pub fn await_pending_poll_for_test(&mut self) -> Result<()> {
        if !self.in_flight {
            return Ok(());
        }
        let result = self
            .result_rx
            .recv()
            .map_err(|err| GlorpError::Message(format!("watch poll worker hung up: {err}")))?;
        self.vm = result?;
        self.poll_count += 1;
        self.in_flight = false;
        Ok(())
    }

    #[doc(hidden)]
    pub fn handle_key_for_test(&mut self, code: KeyCode, kind: KeyEventKind) -> Result<bool> {
        let result = self.handle_key(KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind))?;
        // Mirror the previous synchronous semantics: tests expect that after a
        // manual refresh keypress the new view model is already visible.
        if matches!(code, KeyCode::Char('r')) && kind != KeyEventKind::Release {
            self.await_pending_poll_for_test()?;
        }
        Ok(result)
    }
}

impl Drop for WatchApp {
    fn drop(&mut self) {
        // Best-effort: if the worker is gone the send fails harmlessly. We
        // still join so the thread does not outlive the app.
        let _ = self.request_tx.send(PollRequest::Shutdown);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

pub trait WatchUsagePoller: Send {
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
    render_watch_frame_with_capability(frame, vm, crate::tui::style::ColorCapability::Truecolor);
}

pub fn render_help_overlay_for_test(frame: &mut ratatui::Frame<'_>) {
    render_watch_frame_with_capability(
        frame,
        &WatchViewModel::fixture(),
        crate::tui::style::ColorCapability::Truecolor,
    );
    render_help_overlay(frame);
}

pub fn render_evolution_overlay_for_test(frame: &mut ratatui::Frame<'_>) {
    let vm = WatchViewModel::fixture();
    render_watch_frame_with_capability(frame, &vm, crate::tui::style::ColorCapability::Truecolor);
    render_evolution_overlay(frame, Some(vm.stage.as_str()));
}

pub fn render_hatch_overlay_for_test(frame: &mut ratatui::Frame<'_>) {
    render_watch_frame_with_capability(
        frame,
        &WatchViewModel::fixture(),
        crate::tui::style::ColorCapability::Truecolor,
    );
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
