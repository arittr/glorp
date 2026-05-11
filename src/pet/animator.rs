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

use crate::pet::generation::Species;
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
    /// effects for any detected transitions. Effects are flavored by species
    /// where it matters: glitch flickers harder on mood swings, mech sweeps
    /// in tighter on feed, etc.
    pub fn update(&mut self, vm: &WatchViewModel) {
        let species = Species::parse(&vm.pet_render.generated_species);

        // Hatch: first time we see a freshly-hatched pet, play coalesce.
        if self.first_update && vm.age_days == 0 {
            self.enqueue(EffectKey::Hatch, coalesce(ms(species_hatch_ms(species))));
        }

        // Stage-up: stage label changed. dissolve+coalesce sequence so the
        // pet morphs out and the new one fades in. Glitch dissolves longer
        // for a more chaotic rebirth; mech coalesces tighter.
        if let Some(prev) = &self.last_stage {
            if prev != &vm.stage {
                let (diss, coal) = species_stage_up_ms(species);
                self.enqueue(
                    EffectKey::StageUp,
                    sequence(&[dissolve(ms(diss)), coalesce(ms(coal))]),
                );
            }
        }

        // Mood fade: mood label changed. ~400ms hsl drift toward the new mood,
        // scaled by species (glitch swings harder; ghost barely registers).
        if let Some(prev) = &self.last_mood {
            if prev != &vm.mood {
                let drift = mood_hsl_drift_for(&vm.mood, species);
                self.enqueue(EffectKey::MoodFade, hsl_shift(Some(drift), None, ms(400)));
            }
        }

        // Feed pulse: today's effective tokens jumped by more than the
        // threshold since the last tick. Species-tinted sweep tells you
        // who got fed at a glance.
        if let Some(prev_tokens) = self.last_today_tokens {
            let delta = vm.today_effective_tokens - prev_tokens;
            if delta >= FEED_EVENT_TOKEN_THRESHOLD {
                self.enqueue(
                    EffectKey::FeedPulse,
                    sweep_in(
                        Motion::LeftToRight,
                        10,
                        0,
                        species_feed_color(species),
                        ms(400),
                    ),
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

/// Per-species scaling on `mood_hsl_drift`. Glitch reacts harder, ghost
/// barely shifts (already low-saturation by design), others sit near 1.0.
fn species_mood_drift_scale(species: Option<Species>) -> f32 {
    match species {
        Some(Species::Glitch) => 1.6,
        Some(Species::Ghost) => 0.4,
        Some(Species::Crystal) => 1.2,
        Some(Species::Blob) => 1.1,
        Some(Species::Fuzz) => 1.0,
        Some(Species::Mech) => 0.7,
        None => 1.0,
    }
}

fn mood_hsl_drift_for(mood: &str, species: Option<Species>) -> [f32; 3] {
    let [h, s, l] = mood_hsl_drift(mood);
    let scale = species_mood_drift_scale(species);
    [h * scale, s * scale, l * scale]
}

/// Sweep color used for the feed pulse, flavored per species so each pet
/// looks like itself eating.
fn species_feed_color(species: Option<Species>) -> Color {
    match species {
        Some(Species::Fuzz) => Color::Rgb(255, 200, 150), // warm peach
        Some(Species::Blob) => Color::Rgb(140, 220, 160), // mint
        Some(Species::Ghost) => Color::Rgb(190, 170, 240), // pale lavender
        Some(Species::Glitch) => Color::Rgb(120, 255, 180), // acid green
        Some(Species::Crystal) => Color::Rgb(170, 220, 255), // ice cyan
        Some(Species::Mech) => Color::Rgb(255, 220, 100), // amber
        None => Color::Yellow,
    }
}

/// Hatch coalesce duration. Mech snaps together, blob takes its time.
fn species_hatch_ms(species: Option<Species>) -> u32 {
    match species {
        Some(Species::Mech) => 900,
        Some(Species::Glitch) => 1000,
        Some(Species::Fuzz) | Some(Species::Crystal) => 1200,
        Some(Species::Ghost) => 1400,
        Some(Species::Blob) => 1500,
        None => 1200,
    }
}

/// Stage-up dissolve+coalesce timings. Glitch lingers in the dissolve;
/// mech morphs cleanly; crystal does a slow, deliberate rebuild.
fn species_stage_up_ms(species: Option<Species>) -> (u32, u32) {
    match species {
        Some(Species::Glitch) => (500, 400),
        Some(Species::Crystal) => (300, 700),
        Some(Species::Mech) => (250, 400),
        Some(Species::Blob) => (350, 600),
        Some(Species::Ghost) => (400, 500),
        Some(Species::Fuzz) => (300, 500),
        None => (300, 500),
    }
}

/// Deterministic 0/1 row offset for the pet's idle-breathing animation.
/// Returns 1 only during the brief "peak inhale" window of each cycle so the
/// pet appears to rise on a slow breath and settle back to rest. Period and
/// inhale duration vary per species — glitch breathes jittery and quick,
/// crystal breathes slow and deliberate.
pub fn compute_breath_offset(species: Option<Species>, now: time::OffsetDateTime) -> u8 {
    let (period_ds, inhale_ds) = species_breath_rhythm_decis(species);
    let ts_ds = now.unix_timestamp() * 10 + i64::from(now.millisecond() / 100);
    let phase = ts_ds.rem_euclid(period_ds);
    if phase < inhale_ds {
        1
    } else {
        0
    }
}

/// Per-species breath rhythm in tenths-of-a-second. Returns (period, inhale_window).
/// Glitch: 2.0s cycle, 0.4s peak — twitchy. Crystal: 6.0s cycle, 0.8s peak — slow.
fn species_breath_rhythm_decis(species: Option<Species>) -> (i64, i64) {
    match species {
        Some(Species::Glitch) => (20, 4),
        Some(Species::Mech) => (40, 5),
        Some(Species::Fuzz) => (40, 8),
        Some(Species::Ghost) => (45, 10),
        Some(Species::Blob) => (50, 12),
        Some(Species::Crystal) => (60, 8),
        None => (40, 8),
    }
}

/// Deterministic ±1 column offset for the pet's idle-wander animation.
/// Returns -1, 0, or +1 based on the current wall clock. Tuned to read as
/// slow drift rather than periodic stepping: the pet sits centered for
/// long stretches and takes brief excursions to either side.
///
/// 8-step wave at 8 seconds per step (64s full cycle). Six of the eight
/// steps are 0 (rest), one is +1, one is -1, so the pet is centered ~75%
/// of the time and the excursions feel like gentle floating rather than
/// rhythmic hops.
pub fn compute_wander_offset(now: time::OffsetDateTime) -> i8 {
    let step = now.unix_timestamp().rem_euclid(64) / 8;
    match step {
        2 => 1,
        6 => -1,
        _ => 0,
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
    fn feed_color_differs_per_species() {
        let fuzz = species_feed_color(Some(Species::Fuzz));
        let glitch = species_feed_color(Some(Species::Glitch));
        let mech = species_feed_color(Some(Species::Mech));
        assert_ne!(fuzz, glitch);
        assert_ne!(glitch, mech);
        assert_ne!(fuzz, mech);
    }

    #[test]
    fn mood_drift_scales_per_species() {
        let base = mood_hsl_drift("happy");
        let glitch = mood_hsl_drift_for("happy", Some(Species::Glitch));
        let ghost = mood_hsl_drift_for("happy", Some(Species::Ghost));
        // Glitch should swing harder than baseline; ghost should swing less.
        assert!(glitch[0].abs() > base[0].abs());
        assert!(ghost[0].abs() < base[0].abs());
    }

    #[test]
    fn stage_up_timings_differ_per_species() {
        let glitch = species_stage_up_ms(Some(Species::Glitch));
        let mech = species_stage_up_ms(Some(Species::Mech));
        assert_ne!(glitch, mech);
    }

    #[test]
    fn breath_offset_returns_zero_or_one() {
        use time::macros::datetime;
        let mut saw_zero = false;
        let mut saw_one = false;
        let start = datetime!(2026-05-11 12:00:00 UTC);
        for ms in (0..6000).step_by(100) {
            let v = compute_breath_offset(
                Some(Species::Fuzz),
                start + time::Duration::milliseconds(ms),
            );
            assert!(matches!(v, 0 | 1), "got {v}");
            if v == 0 {
                saw_zero = true;
            } else {
                saw_one = true;
            }
        }
        assert!(saw_zero && saw_one, "should toggle within one cycle");
    }

    #[test]
    fn breath_rhythm_differs_per_species() {
        assert_ne!(
            species_breath_rhythm_decis(Some(Species::Glitch)),
            species_breath_rhythm_decis(Some(Species::Crystal))
        );
    }

    #[test]
    fn wander_offset_returns_one_of_three_values() {
        use time::macros::datetime;
        let mut seen = std::collections::HashSet::new();
        let start = datetime!(2026-05-11 12:00:00 UTC);
        for s in 0..64 {
            let v = compute_wander_offset(start + time::Duration::seconds(s));
            assert!((-1..=1).contains(&v), "got {v}");
            seen.insert(v);
        }
        assert_eq!(seen.len(), 3, "should visit -1, 0, and +1 across a cycle");
    }

    #[test]
    fn wander_rests_at_center_most_of_the_time() {
        use time::macros::datetime;
        let start = datetime!(2026-05-11 12:00:00 UTC);
        let mut rest_seconds = 0;
        for s in 0..64 {
            if compute_wander_offset(start + time::Duration::seconds(s)) == 0 {
                rest_seconds += 1;
            }
        }
        // Should be 75% rest (48 out of 64 seconds) so the motion reads as
        // drift, not as periodic stepping.
        assert!(
            rest_seconds >= 48,
            "expected ≥75% rest, got {rest_seconds}/64"
        );
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
