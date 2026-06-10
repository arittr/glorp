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
use ratatui::{backend::CrosstermBackend, layout::Position, Terminal};

use crate::{
    error::{GlorpError, Result},
    format::format_tokens,
    pet::animator::PetAnimator,
    tui::{
        component::{self, layout_watch_with_context, ComponentLayout, TargetPath, TargetRole},
        layout::{
            pet_effect_rect_from_layout, render_evolution_overlay, render_hatch_overlay,
            render_help_overlay, render_watch_frame_with_capability,
            render_watch_frame_with_layout,
        },
        render_context::RenderContext,
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

const EVOLUTION_OVERLAY_HOLD: Duration = Duration::from_secs(6);

pub struct WatchApp {
    vm: WatchViewModel,
    config: WatchAppConfig,
    overlay: Option<Overlay>,
    request_tx: Sender<PollRequest>,
    result_rx: Receiver<Result<WatchPollResult>>,
    worker: Option<JoinHandle<()>>,
    life_signal_state: crate::tui::life::LifeSignalState,
    in_flight: bool,
    poll_count: u64,
    animation_frame: u64,
    last_poll: Option<Instant>,
    last_acknowledged_evolution: Option<String>,
    evolution_overlay_started_at: Option<Instant>,
    pet_animator: PetAnimator,
    last_frame_time: Option<Instant>,
    /// Wall-clock instant of the last 'p' press; drives a transient speech
    /// bubble override and happiness bump in the watch view.
    pet_petted_at: Option<Instant>,
    /// The phrase chosen at the moment of the last 'p' press, held until
    /// the petting bubble window expires.
    petting_phrase: Option<String>,
    /// The value most recently passed to `rerender_pet_for_view_model` as the
    /// sleep-presentation `hold_eyes_closed` flag. Tracked so tests can assert
    /// the milestone exemption (evolution overlay keeps eyes open).
    last_hold_eyes_closed: bool,
}

/// Faster tick rate used while tachyonfx effects are active. ~60 fps target.
const FAST_TICK: Duration = Duration::from_millis(16);

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
        let (result_tx, result_rx) = mpsc::channel::<Result<WatchPollResult>>();
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
        // Treat the stage the pet starts at as already-acknowledged so the
        // overlay only fires for evolutions that happen during this session.
        // Otherwise re-opening the app shows the popup every time for a pet
        // that evolved long ago.
        let last_acknowledged_evolution = vm.latest_evolution.clone();
        Self {
            vm,
            config,
            overlay: None,
            request_tx,
            result_rx,
            worker: Some(worker),
            life_signal_state: crate::tui::life::LifeSignalState::default(),
            in_flight: false,
            poll_count: 0,
            animation_frame: 0,
            last_poll: None,
            last_acknowledged_evolution,
            evolution_overlay_started_at: None,
            pet_animator: PetAnimator::new(),
            last_frame_time: None,
            pet_petted_at: None,
            petting_phrase: None,
            last_hold_eyes_closed: false,
        }
    }

    pub fn run(mut self) -> Result<()> {
        // Detect terminal background BEFORE EnableMouseCapture / EnterAlternateScreen;
        // terminal-light queries OSC 11 against the host terminal and the
        // response could be lost once we've taken over the screen.
        let theme = crate::tui::style::detect_theme();
        crate::tui::style::init_theme(theme);

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
        self.start_initial_poll()?;
        loop {
            self.advance_animation_frame();

            // Drain any completed poll result before drawing so the new vm is
            // visible this frame.
            self.try_collect_poll_result()?;

            // Reassert any in-flight petting bubble after the worker poll
            // may have replaced vm.current_speech.
            self.apply_pet_petted_override();

            // Update the pet animator with the latest view model. This may
            // enqueue mood-fade / stage-up / feed-pulse / hatch effects.
            self.pet_animator.update(&self.vm);

            let now = Instant::now();
            let elapsed_ms = self
                .last_frame_time
                .map(|t| now.duration_since(t).as_millis() as u32)
                .unwrap_or(0);
            self.last_frame_time = Some(now);

            let render_evolution = self.update_evolution_overlay();
            let stage_label = self.vm.stage.clone();
            let vm_ref = &self.vm;
            let ctx = RenderContext::new(self.config.color_capability);
            let overlay = self.overlay;
            let animator = &mut self.pet_animator;
            let mut rendered_layout = None;
            terminal.draw(|frame| {
                let frame_area = frame.area();
                let layout = layout_watch_with_context(frame_area, vm_ref, &ctx);
                render_watch_frame_with_layout(frame, vm_ref, &ctx, &layout);
                // Apply tachyonfx effects on top of the rendered pet panel.
                let pet_rect = pet_effect_rect_from_layout(&layout);
                animator.apply(pet_rect, frame.buffer_mut(), elapsed_ms);
                rendered_layout = Some(layout);
                match overlay {
                    Some(Overlay::Help) => render_help_overlay(frame),
                    None => {}
                }
                if render_evolution {
                    render_evolution_overlay(frame, Some(stage_label.as_str()));
                }
            })?;
            let rendered_layout =
                rendered_layout.expect("terminal draw closure always records watch layout");

            // Two-rate tick: while effects are active, poll at 60 fps so the
            // animation looks smooth. Otherwise use the configured idle tick.
            let tick = if self.pet_animator.has_active_effects() {
                FAST_TICK
            } else {
                self.config.animation_tick
            };
            if event::poll(tick)? {
                match event::read()? {
                    Event::Key(key) if self.handle_key(key)? => break,
                    Event::Mouse(mouse) => self.handle_mouse(mouse, &rendered_layout),
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

    fn compute_hold_eyes_closed(
        vm: &WatchViewModel,
        overlay: &Option<Overlay>,
        evolution_overlay_started_at: &Option<Instant>,
    ) -> bool {
        vm.day_context.asleep && evolution_overlay_started_at.is_none() && overlay.is_none()
    }

    fn advance_animation_frame(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1);
        let hold_eyes_closed = Self::compute_hold_eyes_closed(
            &self.vm,
            &self.overlay,
            &self.evolution_overlay_started_at,
        );
        self.last_hold_eyes_closed = hold_eyes_closed;
        let _ = crate::commands::watch::rerender_pet_for_view_model(
            &mut self.vm,
            self.animation_frame,
            hold_eyes_closed,
        );
        let now = time::OffsetDateTime::now_utc();
        let species = self.vm.pet_render.generated_species;
        // wander_offset_x and facing are computed at render time in the panel
        // from area.width so they stay mutually consistent. We only update
        // breath and feed-pulse timestamp here.
        self.vm.breath_offset_y = crate::pet::animator::compute_breath_offset(Some(species), now);
        self.vm.last_feed_pulse_at = self.pet_animator.last_feed_pulse_at;
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
            KeyCode::Char('p') => {
                self.pet_the_pet();
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    /// Trigger the petting interaction: choose a reaction phrase, override
    /// the visible speech bubble for a few seconds, and give vitals a small
    /// transient lift. Non-persistent — the next worker poll restores the
    /// real PetState happiness/energy from disk.
    fn pet_the_pet(&mut self) {
        let now_wall = time::OffsetDateTime::now_utc();
        let phrase = crate::pet::speech::pick_petting_phrase(now_wall);
        self.vm.current_speech = Some(phrase.clone());
        self.petting_phrase = Some(phrase);
        self.pet_petted_at = Some(Instant::now());
        self.vm.happiness = (self.vm.happiness + 0.08).min(1.0);
        self.vm.energy = (self.vm.energy + 0.04).min(1.0);
    }

    /// Reassert the petting speech override on each frame while the bubble
    /// window is still open. Without this, the regular worker poll would
    /// replace `vm.current_speech` with the mood-derived line.
    fn apply_pet_petted_override(&mut self) {
        let Some(started_at) = self.pet_petted_at else {
            return;
        };
        if started_at.elapsed() < crate::pet::speech::PETTING_BUBBLE_VISIBLE {
            if let Some(phrase) = self.petting_phrase.as_deref() {
                self.vm.current_speech = Some(phrase.to_string());
            }
        } else {
            self.pet_petted_at = None;
            self.petting_phrase = None;
        }
    }

    /// Update vm.cursor_screen from a crossterm MouseEvent so PetPanel can
    /// swap to cursor-tracked eyes on its next render, but only while the
    /// pointer is over the pet scene's declared hit target.
    fn handle_mouse(&mut self, mouse: MouseEvent, layout: &ComponentLayout) {
        if !self.vm.mouse_tracking_enabled {
            return;
        }
        match mouse.kind {
            MouseEventKind::Moved
            | MouseEventKind::Drag(_)
            | MouseEventKind::Down(_)
            | MouseEventKind::Up(_) => {
                let point = Position::new(mouse.column, mouse.row);
                let over_pet_hit_area = component::hit_test(layout, point)
                    .and_then(|hit| layout.target(hit.target).map(|target| (hit, target)))
                    .map(|(hit, target)| {
                        hit.target == TargetPath::new("watch.pet.hit")
                            && target.role == TargetRole::HitArea
                    })
                    .unwrap_or(false);

                self.vm.cursor_screen = if over_pet_hit_area {
                    Some((mouse.column, mouse.row))
                } else {
                    None
                };
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

    fn start_initial_poll(&mut self) -> Result<bool> {
        if self.last_poll.is_some() {
            return Ok(false);
        }
        let was_in_flight = self.in_flight;
        self.kick_off_poll()?;
        self.last_poll = Some(Instant::now());
        Ok(!was_in_flight)
    }

    /// Non-blocking collection of any completed poll result.
    fn try_collect_poll_result(&mut self) -> Result<()> {
        match self.result_rx.try_recv() {
            Ok(result) => {
                self.install_poll_result(result?);
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
        let hold_eyes_closed = Self::compute_hold_eyes_closed(
            &self.vm,
            &self.overlay,
            &self.evolution_overlay_started_at,
        );
        self.last_hold_eyes_closed = hold_eyes_closed;
        let _ = crate::commands::watch::rerender_pet_for_view_model(
            &mut self.vm,
            self.animation_frame,
            hold_eyes_closed,
        );
    }

    pub fn view_model_for_test(&self) -> &WatchViewModel {
        &self.vm
    }

    #[cfg(test)]
    pub fn current_vm_for_test_mut(&mut self) -> &mut WatchViewModel {
        &mut self.vm
    }

    #[cfg(test)]
    pub fn last_hold_eyes_closed_for_test(&self) -> bool {
        self.last_hold_eyes_closed
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

    #[doc(hidden)]
    pub fn start_initial_poll_for_test(&mut self) -> Result<bool> {
        self.start_initial_poll()
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
        self.install_poll_result(result?);
        self.poll_count += 1;
        self.in_flight = false;
        Ok(())
    }

    fn install_poll_result(&mut self, mut result: WatchPollResult) -> WatchViewModel {
        let now = time::OffsetDateTime::now_utc();
        let profile = self.life_signal_state.observe(result.applied_signal, now);
        result.vm.life_profile = profile;
        // Night calm: full quiet only while the pet actually sleeps (spec:
        // calm_mode = night && asleep; asleep already implies Night). Must be
        // set after observe (which hardcodes calm_mode: false) and before any
        // profile consumer in this install path.
        result.vm.life_profile.calm_mode = result.vm.day_context.asleep;
        result.vm.last_feed_pulse_at = if result.vm.life_profile.burst_level > 0.0
            && !matches!(
                self.config.color_capability,
                crate::tui::style::ColorCapability::Flat
            ) {
            Some(now)
        } else {
            None
        };
        result.vm.current_speech = crate::pet::speech::current_pet_speech_for_profile(
            result.vm.pet_render.mood,
            &result.vm.life_profile,
            now,
        );
        append_profile_pet_activities(&mut result.vm, now);
        self.vm = result.vm;
        self.vm.clone()
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
        // Signal shutdown and detach the worker rather than joining. A slow
        // helper subprocess (Node startup, network call, fs lock) can keep
        // an in-flight poll parked for many seconds; the Shutdown message
        // sits behind it in the channel queue, so a blocking join would hang
        // the user's terminal after they press `q`. Detaching is safe
        // because the save boundary in `apply_unapplied_usage` plus the
        // unapplied-row ledger already reconciles any in-flight work on the
        // next successful run, and the OS reaps the worker (and any child
        // process it spawned) when glorp exits.
        let _ = self.request_tx.send(PollRequest::Shutdown);
        // Take the handle out of the option but deliberately do not join.
        let _ = self.worker.take();
    }
}

#[derive(Debug, Clone)]
pub struct WatchPollResult {
    pub vm: WatchViewModel,
    pub applied_signal: crate::tui::life::AppliedUsageSignal,
}

pub trait WatchUsagePoller: Send {
    fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchPollResult>;
}

struct NoopWatchPoller;

impl WatchUsagePoller for NoopWatchPoller {
    fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchPollResult> {
        Ok(WatchPollResult {
            vm: current.clone(),
            applied_signal: crate::tui::life::AppliedUsageSignal::quiet(
                time::OffsetDateTime::now_utc(),
                time::Duration::seconds(0),
            ),
        })
    }
}

fn append_profile_pet_activities(vm: &mut WatchViewModel, now: time::OffsetDateTime) {
    let activities = crate::pet::activity::derive_profile_pet_activities(
        &vm.pet_name,
        vm.pet_render.generated_species,
        vm.pet_render.mood,
        &vm.life_profile,
        now,
    );
    for activity in activities {
        if !vm
            .recent_events
            .iter()
            .any(|event| event.kind == activity.kind && event.text == activity.text)
        {
            vm.recent_events.push(activity);
        }
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
    fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchPollResult> {
        self.vm = current.clone();
        let vm = run_single_watch_tick_for_test(self)?;
        Ok(WatchPollResult {
            vm,
            applied_signal: crate::tui::life::AppliedUsageSignal {
                applied_effective_tokens: self.effective_delta.max(0.0),
                raw_effective_tokens: Some(self.effective_delta.max(0.0)),
                source_mix: None,
                token_shape: None,
                observed_at: time::OffsetDateTime::now_utc(),
                elapsed_since_successful_poll: time::Duration::seconds(10),
                freshness: crate::tui::life::UsageSignalFreshness::Live,
            },
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::component::{
        ComponentLayout, ComponentNodeLayout, GeometryTarget, LayoutMode, TargetPath, TargetRole,
        WatchComponentId,
    };
    use crossterm::event::KeyModifiers;
    use ratatui::layout::Rect;
    use time::macros::datetime;

    struct SignalPoller {
        signal: crate::tui::life::AppliedUsageSignal,
    }

    impl WatchUsagePoller for SignalPoller {
        fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchPollResult> {
            Ok(WatchPollResult {
                vm: current.clone(),
                applied_signal: self.signal,
            })
        }
    }

    #[test]
    fn refresh_stamps_life_profile_from_applied_signal_after_session_primer() {
        let now = time::macros::datetime!(2026-06-05 12:00 UTC);
        let vm = WatchViewModel::fixture();
        let signal = crate::tui::life::AppliedUsageSignal {
            applied_effective_tokens: 80_000.0,
            raw_effective_tokens: Some(80_000.0),
            source_mix: None,
            token_shape: None,
            observed_at: now,
            elapsed_since_successful_poll: time::Duration::seconds(10),
            freshness: crate::tui::life::UsageSignalFreshness::Live,
        };
        let config = WatchAppConfig {
            color_capability: crate::tui::style::ColorCapability::Truecolor,
            ..Default::default()
        };
        let mut app = WatchApp::with_poll_callback(vm, config, Box::new(SignalPoller { signal }));

        let primed = app.refresh_for_test().unwrap();
        assert_eq!(primed.life_profile.burst_level, 0.0);
        assert_eq!(primed.last_feed_pulse_at, None);

        let refreshed = app.refresh_for_test().unwrap();
        assert!(refreshed.life_profile.activity_level > 0.0);
        assert!(refreshed.life_profile.burst_level > 0.0);
        assert!(refreshed.last_feed_pulse_at.is_some());
    }

    #[test]
    fn flat_mode_does_not_stamp_feed_pulse_for_live_burst() {
        let now = time::macros::datetime!(2026-06-05 12:00 UTC);
        let vm = WatchViewModel::fixture();
        let signal = crate::tui::life::AppliedUsageSignal {
            applied_effective_tokens: 80_000.0,
            raw_effective_tokens: Some(80_000.0),
            source_mix: None,
            token_shape: None,
            observed_at: now,
            elapsed_since_successful_poll: time::Duration::seconds(10),
            freshness: crate::tui::life::UsageSignalFreshness::Live,
        };
        let config = WatchAppConfig {
            color_capability: crate::tui::style::ColorCapability::Flat,
            ..Default::default()
        };
        let mut app = WatchApp::with_poll_callback(vm, config, Box::new(SignalPoller { signal }));

        app.refresh_for_test().unwrap();
        let refreshed = app.refresh_for_test().unwrap();

        assert!(refreshed.life_profile.burst_level > 0.0);
        assert_eq!(refreshed.last_feed_pulse_at, None);
    }

    #[test]
    fn refresh_adds_profile_activity_without_replacing_usage_rows() {
        let mut app = WatchApp::with_poll_callback(
            WatchViewModel::fixture(),
            Default::default(),
            Box::new(WatchTestHarness::with_usage_delta(
                "codex",
                "2026-06-05T13:42:00Z",
                80_000.0,
            )),
        );

        let primed = app.refresh_for_test().unwrap();
        assert!(
            primed
                .recent_events
                .iter()
                .any(|event| event.kind == LogKind::Usage
                    && event.text.contains("codex added 80.0k effective tokens")),
            "usage row should remain in the feed: {:?}",
            primed.recent_events
        );

        let refreshed = app.refresh_for_test().unwrap();

        assert!(
            refreshed
                .recent_events
                .iter()
                .any(|event| event.kind == LogKind::Usage
                    && event.text.contains("codex added 80.0k effective tokens")),
            "usage row should remain in the feed: {:?}",
            refreshed.recent_events
        );
        assert!(
            refreshed.recent_events.iter().any(|event| {
                event.kind == LogKind::PetActivity && event.text.contains(&refreshed.pet_name)
            }),
            "profile activity line should be appended: {:?}",
            refreshed.recent_events
        );
    }

    #[test]
    fn evolution_overlay_does_not_fire_for_stage_that_predates_app_launch() {
        // vm.latest_evolution carries the pet's current stage, not "an evolution
        // just happened". On app startup an existing stage must be treated as
        // already-acknowledged so the overlay doesn't fire every time you open
        // the app for a pet that evolved sessions ago.
        let mut vm = WatchViewModel::fixture();
        vm.latest_evolution = Some("fuzz".into());
        let mut app = WatchApp::new(vm);
        assert!(
            !app.update_evolution_overlay(),
            "overlay must not fire on launch for a stage the user has already seen"
        );
    }

    #[test]
    fn evolution_overlay_hold_is_long_enough_to_read_three_lines() {
        // 3s wasn't enough to read three lines of celebratory copy in casual
        // testing. ≥5s gives the overlay time to land before it vanishes.
        assert!(
            EVOLUTION_OVERLAY_HOLD >= Duration::from_secs(5),
            "EVOLUTION_OVERLAY_HOLD is {:?}; needs ≥5s to read",
            EVOLUTION_OVERLAY_HOLD
        );
    }

    #[test]
    fn handle_mouse_tracks_cursor_only_over_pet_hit_area() {
        let layout = test_layout_with_pet_hit_area();
        let mut app = WatchApp::new(WatchViewModel::fixture());

        app.handle_mouse(mouse_event(MouseEventKind::Moved, 14, 9), &layout);

        assert_eq!(app.view_model_for_test().cursor_screen, Some((14, 9)));
    }

    #[test]
    fn handle_mouse_clears_cursor_when_mouse_moves_outside_pet_hit_area() {
        let layout = test_layout_with_pet_hit_area();
        let mut app = WatchApp::new(WatchViewModel::fixture());

        app.handle_mouse(mouse_event(MouseEventKind::Moved, 14, 9), &layout);
        app.handle_mouse(mouse_event(MouseEventKind::Moved, 10, 5), &layout);

        assert_eq!(app.view_model_for_test().cursor_screen, None);
    }

    fn test_app_with_signal(signal: crate::tui::life::AppliedUsageSignal) -> WatchApp {
        WatchApp::with_poll_callback(
            WatchViewModel::fixture(),
            WatchAppConfig::default(),
            Box::new(SignalPoller { signal }),
        )
    }

    #[test]
    fn asleep_day_context_sets_calm_mode_and_it_survives_the_poll_install() {
        // Build a WatchApp via the SignalPoller test harness with a vm whose
        // day_context.asleep is true in the poll result.
        let mut app = test_app_with_signal(crate::tui::life::AppliedUsageSignal::quiet(
            datetime!(2026-06-09 23:30 UTC),
            time::Duration::seconds(10),
        ));
        // SignalPoller returns current.clone(), so mutating the fixture VM carries into the poll result.
        app.current_vm_for_test_mut().day_context.asleep = true;
        app.refresh_for_test().unwrap();
        // First-poll establishment: the very first install must already be calm.
        assert!(
            app.view_model_for_test().life_profile.calm_mode,
            "asleep must engage calm_mode on the installed profile"
        );

        app.current_vm_for_test_mut().day_context.asleep = false;
        app.refresh_for_test().unwrap();
        assert!(
            !app.view_model_for_test().life_profile.calm_mode,
            "asleep=false must disengage calm_mode"
        );
    }

    fn test_layout_with_pet_hit_area() -> ComponentLayout {
        let owner = WatchComponentId::Pet.path();
        let mut layout = ComponentLayout::new(Rect::new(0, 0, 80, 24), LayoutMode::Wide);
        layout
            .insert_node(ComponentNodeLayout::leaf(owner, Rect::new(10, 5, 20, 10)))
            .unwrap();
        layout
            .insert_target(
                TargetPath::new("watch.pet.panel"),
                GeometryTarget {
                    owner,
                    rect: Rect::new(10, 5, 20, 10),
                    z: 0,
                    clip: Rect::new(10, 5, 20, 10),
                    role: TargetRole::PetPanel,
                },
            )
            .unwrap();
        layout
            .insert_target(
                TargetPath::new("watch.pet.hit"),
                GeometryTarget {
                    owner,
                    rect: Rect::new(12, 7, 6, 5),
                    z: 30,
                    clip: Rect::new(12, 7, 6, 5),
                    role: TargetRole::HitArea,
                },
            )
            .unwrap();
        layout
    }

    fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn evolution_overlay_renders_the_pet_awake_even_while_asleep() {
        // Milestone exemption: while the evolution overlay window is active,
        // hold_eyes_closed must be false despite day_context.asleep.
        let mut app = WatchApp::new(WatchViewModel::fixture());
        {
            let vm = app.current_vm_for_test_mut();
            vm.day_context.asleep = true;
            vm.latest_evolution = Some("fuzz".into());
        }
        // Fire the evolution overlay so started_at is set.
        assert!(app.update_evolution_overlay(), "overlay should be active");
        app.advance_animation_for_test();
        assert!(
            !app.last_hold_eyes_closed_for_test(),
            "evolution overlay must suppress hold_eyes_closed even while asleep"
        );
    }

    #[test]
    fn help_overlay_renders_the_pet_awake_even_while_asleep() {
        let mut app = test_app_with_signal(crate::tui::life::AppliedUsageSignal::quiet(
            datetime!(2026-06-09 23:30 UTC),
            time::Duration::seconds(10),
        ));
        app.current_vm_for_test_mut().day_context.asleep = true;
        app.overlay = Some(Overlay::Help);
        app.advance_animation_for_test();
        assert!(
            !app.last_hold_eyes_closed_for_test(),
            "help overlay must keep eyes open even while asleep"
        );
    }
}
