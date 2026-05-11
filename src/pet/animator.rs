//! Pet animation orchestration: detects state transitions on the view model
//! and enqueues tachyonfx effects that play on top of the rendered pet panel.
//!
//! The animator is owned by `WatchApp`. Each frame:
//! 1. `update(vm, elapsed)` inspects the current view model, detects changes
//!    since the previous frame (mood, stage, usage spike), and enqueues the
//!    appropriate tachyonfx Effect via the internal EffectManager.
//! 2. `apply(area, buf)` is called from inside the dispatcher's `Frame::draw`
//!    closure after the panel renders. It advances the manager by the same
//!    `elapsed` and mutates the buffer cells the panel just wrote.
//! 3. `has_active_effects()` lets the watch loop pick the faster tick rate
//!    while transitions are in flight.
//!
//! Effects in scope for this initial Phase 4 cut:
//! - Mood fade (`hsl_shift` over ~400ms when `vm.mood` changes)
//! - Stage-up morph (`dissolve` + `coalesce` ~800ms when `vm.stage` changes)
//! - Feed pulse (`sweep_in` accent wash ~400ms when today's tokens jump)
//! - Hatch sequence (`coalesce` of the s0 art ~1.2s on first frame for a
//!   freshly-hatched pet — detected via `age_days == 0` and a fresh animator)
//!
//! Low-energy continuous droop is implemented as a direct palette shift in
//! the panel renderer, not as a tachyonfx effect — it's a steady-state visual,
//! not an animation.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use tachyonfx::fx::{coalesce, dissolve, hsl_shift, sequence, sweep_in};
use tachyonfx::{Duration as FxDuration, Effect, EffectManager, Motion};

fn ms(n: u32) -> FxDuration {
    FxDuration::from_millis(n)
}

use crate::tui::view_model::WatchViewModel;

/// Threshold (in tokens) above which a single tick of usage growth is
/// treated as a "feed event" and triggers a pulse effect.
const FEED_EVENT_TOKEN_THRESHOLD: f64 = 250.0;

/// Per-effect key. Used by `add_unique_effect` so a new transition of the
/// same kind cancels the previous one in flight (e.g., mood changing twice
/// in quick succession only plays the most recent fade).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
enum EffectKey {
    #[default]
    Idle,
    Hatch,
    StageUp,
    MoodFade,
    FeedPulse,
}

/// Longest effect duration we schedule (hatch is 1.2s; allow buffer for
/// sequence(dissolve+coalesce) which is 0.8s total). After this many ms
/// since the most recent enqueue with no new enqueues, we declare the
/// animator quiescent and the watch loop returns to the slow tick rate.
const MAX_EFFECT_RUNTIME_MS: u32 = 1500;

pub struct PetAnimator {
    manager: EffectManager<EffectKey>,
    /// Mood seen on the previous `update`. None until first call.
    last_mood: Option<String>,
    /// Stage seen on the previous `update`. None until first call.
    last_stage: Option<String>,
    /// Today's effective tokens seen on the previous `update`.
    last_today_tokens: Option<f64>,
    /// True until the first `update` call seeds the snapshot. Used to fire
    /// the hatch effect on the initial frame for a pet whose age == 0.
    first_update: bool,
    /// Milliseconds since the most recent enqueue. While < MAX_EFFECT_RUNTIME_MS
    /// we treat the animator as running and the watch loop uses the fast
    /// tick rate. EffectManager 0.18 doesn't expose a running query, so this
    /// approximation is the simplest reliable way to drive two-rate ticking.
    idle_ms: u32,
    /// True once at least one effect has been enqueued.
    has_run: bool,
}

impl PetAnimator {
    pub fn new() -> Self {
        Self {
            manager: EffectManager::default(),
            last_mood: None,
            last_stage: None,
            last_today_tokens: None,
            first_update: true,
            idle_ms: MAX_EFFECT_RUNTIME_MS,
            has_run: false,
        }
    }

    fn enqueue(&mut self, key: EffectKey, fx: Effect) {
        self.manager.add_unique_effect(key, fx);
        self.idle_ms = 0;
        self.has_run = true;
    }

    /// Diff the view model against the previous frame's snapshot and enqueue
    /// effects for any detected transitions.
    pub fn update(&mut self, vm: &WatchViewModel) {
        // Hatch: first time we see a freshly-hatched pet, play coalesce.
        if self.first_update && vm.age_days == 0 {
            self.enqueue(EffectKey::Hatch, coalesce(ms(1200)));
        }

        // Stage-up: stage label changed. dissolve+coalesce sequence so the
        // pet morphs out and the new one fades in.
        if let Some(prev) = &self.last_stage {
            if prev != &vm.stage {
                self.enqueue(
                    EffectKey::StageUp,
                    sequence(&[dissolve(ms(300)), coalesce(ms(500))]),
                );
            }
        }

        // Mood fade: mood label changed. ~400ms hsl drift toward the new mood.
        if let Some(prev) = &self.last_mood {
            if prev != &vm.mood {
                let drift = mood_hsl_drift(&vm.mood);
                self.enqueue(EffectKey::MoodFade, hsl_shift(Some(drift), None, ms(400)));
            }
        }

        // Feed pulse: today's effective tokens jumped by more than the
        // threshold since the last tick. Sweep an accent wash across the
        // pet to signal that food arrived.
        if let Some(prev_tokens) = self.last_today_tokens {
            let delta = vm.today_effective_tokens - prev_tokens;
            if delta >= FEED_EVENT_TOKEN_THRESHOLD {
                self.enqueue(
                    EffectKey::FeedPulse,
                    sweep_in(Motion::LeftToRight, 10, 0, Color::Yellow, ms(400)),
                );
            }
        }

        self.last_mood = Some(vm.mood.clone());
        self.last_stage = Some(vm.stage.clone());
        self.last_today_tokens = Some(vm.today_effective_tokens);
        self.first_update = false;
    }

    /// Render any active effects on top of `buf` within `area`. Called from
    /// the dispatcher after the pet panel has rendered its base content.
    /// `elapsed_ms` is the milliseconds since the previous apply.
    pub fn apply(&mut self, area: Rect, buf: &mut Buffer, elapsed_ms: u32) {
        self.manager.process_effects(ms(elapsed_ms), buf, area);
        self.idle_ms = self.idle_ms.saturating_add(elapsed_ms);
    }

    /// Whether any effect is currently running. True for `MAX_EFFECT_RUNTIME_MS`
    /// after the most recent enqueue. The watch loop uses this to pick the
    /// faster tick rate while transitions are in flight.
    pub fn has_active_effects(&self) -> bool {
        self.has_run && self.idle_ms < MAX_EFFECT_RUNTIME_MS
    }
}

impl Default for PetAnimator {
    fn default() -> Self {
        Self::new()
    }
}

/// HSL drift applied during the mood-fade effect. Values are deltas to
/// current cell color: [hue_degrees, saturation_pct, lightness_pct].
/// Sleepy: cooler and dimmer. Happy: warmer and brighter. Etc.
fn mood_hsl_drift(mood: &str) -> [f32; 3] {
    match mood {
        "sleepy" => [-20.0, -25.0, -15.0],
        "hungry" => [10.0, 10.0, -5.0],
        "sad" => [-10.0, -20.0, -20.0],
        "happy" => [15.0, 20.0, 10.0],
        "wilted" => [0.0, -40.0, -30.0],
        _ => [0.0, 0.0, 0.0], // content / unknown
    }
}

/// Multiplier applied to the body palette role lightness when energy is low.
/// Returns 1.0 (no change) at energy >= 0.6, falling linearly to 0.55 at
/// energy == 0.0. The renderer multiplies the body's foreground color by
/// this factor to produce the visible droop. Phase 4 implements droop as a
/// steady-state shader on the panel, not a tachyonfx effect, because it's
/// a continuous attribute of the pet rather than a transition.
pub fn low_energy_lightness_multiplier(energy: f64) -> f32 {
    if energy >= 0.6 {
        1.0
    } else {
        let t = (energy / 0.6).clamp(0.0, 1.0) as f32;
        // Lerp from 0.55 at t=0 to 1.0 at t=1.
        0.55 + 0.45 * t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_vm() -> WatchViewModel {
        WatchViewModel::fixture()
    }

    #[test]
    fn new_animator_has_no_active_effects() {
        let a = PetAnimator::new();
        assert!(!a.has_active_effects());
    }

    #[test]
    fn first_update_for_age_zero_pet_enqueues_hatch() {
        let mut a = PetAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 0;
        a.update(&vm);
        assert!(a.has_active_effects(), "hatch should be queued");
    }

    #[test]
    fn first_update_for_older_pet_does_not_hatch() {
        let mut a = PetAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 5;
        a.update(&vm);
        assert!(!a.has_active_effects(), "no hatch for an older pet");
    }

    #[test]
    fn mood_change_enqueues_fade() {
        let mut a = PetAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 5; // avoid hatch
        vm.mood = "content".into();
        a.update(&vm);
        assert!(!a.has_active_effects(), "no effects after initial snapshot");

        vm.mood = "sleepy".into();
        a.update(&vm);
        assert!(a.has_active_effects(), "mood change should fire fade");
    }

    #[test]
    fn stage_change_enqueues_stage_up() {
        let mut a = PetAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 5;
        vm.stage = "hatchling".into();
        a.update(&vm);
        vm.stage = "juvenile".into();
        a.update(&vm);
        assert!(a.has_active_effects(), "stage change should fire stage-up");
    }

    #[test]
    fn feed_pulse_fires_on_token_spike() {
        let mut a = PetAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 5;
        vm.today_effective_tokens = 1_000.0;
        a.update(&vm);
        vm.today_effective_tokens = 5_000.0; // +4000 > threshold
        a.update(&vm);
        assert!(a.has_active_effects(), "feed should pulse");
    }

    #[test]
    fn small_token_growth_does_not_fire_feed_pulse() {
        let mut a = PetAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 5;
        vm.today_effective_tokens = 1_000.0;
        a.update(&vm);
        vm.today_effective_tokens = 1_050.0; // +50, below threshold
        a.update(&vm);
        assert!(!a.has_active_effects(), "tiny growth should not pulse");
    }

    #[test]
    fn low_energy_multiplier_clamps() {
        assert_eq!(low_energy_lightness_multiplier(0.7), 1.0);
        assert_eq!(low_energy_lightness_multiplier(0.6), 1.0);
        let mid = low_energy_lightness_multiplier(0.3);
        assert!(mid > 0.55 && mid < 1.0, "got {mid}");
        assert_eq!(low_energy_lightness_multiplier(0.0), 0.55);
    }
}
