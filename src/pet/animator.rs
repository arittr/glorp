//! Pet animation orchestration: detects state transitions on the view model
//! and enqueues tachyonfx effects that play on top of the rendered watch frame.
//!
//! The animator is owned by `WatchApp`. Each frame:
//! 1. `update(vm, targets)` inspects the current view model, detects changes
//!    since the previous frame (mood, stage, live profile burst, room scene
//!    moments), and enqueues the appropriate tachyonfx Effect via the internal
//!    EffectManager.
//! 2. `apply(targets, buf)` is called from inside the dispatcher's `Frame::draw`
//!    closure after the panel renders. It advances the manager by the same
//!    `elapsed` and mutates the buffer cells the panel just wrote.
//! 3. `has_active_effects()` lets the watch loop pick the faster tick rate
//!    while transitions are in flight.
//!
//! Effects in scope for this initial Phase 4 cut:
//! - Mood fade (`hsl_shift` over ~400ms when `vm.mood` changes)
//! - Stage-up morph (`dissolve` + `coalesce` ~800ms when `vm.stage` changes)
//! - Feed pulse (`sweep_in` accent wash ~400ms when the live profile stamps a burst)
//! - Hatch sequence (`coalesce` of the s0 art ~1.2s on first frame for a
//!   freshly-hatched pet — detected via `age_days == 0` and a fresh animator)
//! - Room scene moments (finite effects keyed by `SceneMomentKey` that play on
//!   room slices, pet area, or prop targets)
//!
//! Low-energy continuous droop is implemented as a direct palette shift in
//! the panel renderer, not as a tachyonfx effect — it's a steady-state visual,
//! not an animation.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::collections::HashSet;

use tachyonfx::fx::{coalesce, dissolve, hsl_shift, sequence, sweep_in};
use tachyonfx::{Duration as FxDuration, Effect, EffectManager, Motion};

fn ms(n: u32) -> FxDuration {
    FxDuration::from_millis(n)
}

use crate::pet::generation::Species;
use crate::pet::render::PaletteRoleName;
use crate::tui::view_model::WatchViewModel;

/// PET_W must match the constant in `tui/panels/pet.rs`.
const PET_W: u16 = 13;
/// How many seconds to hold each drift target before picking a new one.
const TARGET_HOLD_SECS: i64 = 30;
/// How many seconds of shimmer per period.
const SHIMMER_DURATION_SECS: i64 = 1;
/// Shimmer period: a brief tint fires for ~1s every 22s.
const SHIMMER_PERIOD_SECS: i64 = 22;
/// Twinkle period: a sparkle glyph fires for ~1 frame every 7s.
const TWINKLE_PERIOD_SECS: i64 = 7;
/// Sleep onset/wake position easing window, seconds. Also consumed by
/// tui::day's wake-resume derivation.
pub const WANDER_SETTLE_SECS: i64 = 8;
/// Asleep breath: period x3, inhale window x2 — slow and deep.
const SLEEP_BREATH_PERIOD_SCALE: i64 = 3;
const SLEEP_BREATH_INHALE_SCALE: i64 = 2;
pub const TIRED_BREATH_MAX_SCALE: f64 = 1.5;
const TIRED_BREATH_MIN_TIREDNESS: f32 = 0.05;

/// Per-effect key. Used by `add_unique_effect` so a new transition of the
/// same kind cancels the previous one in flight (e.g., mood changing twice
/// in quick succession only plays the most recent fade).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
enum EffectKey {
    #[default]
    Idle,
    Hatch,
    StageUp,
    MoodFade,
    FeedPulse,
    Scene(SceneEffectKey),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SceneEffectKey {
    pub moment: crate::tui::room::SceneMomentKey,
    pub trigger_id: crate::tui::room::SceneTriggerId,
}

/// Target areas for scene effects, derived from the component layout.
#[derive(Default)]
pub struct SceneEffectTargets {
    pub frame: Rect,
    pub pet: Rect,
    /// Whether the layout contained a `watch.room.effect` target.
    pub room_present: bool,
    pub room_slices: Vec<Rect>,
    pub props: std::collections::BTreeMap<&'static str, Rect>,
}

pub struct SceneAnimator {
    manager: EffectManager<EffectKey>,
    /// Mood seen on the previous `update`. None until first call.
    last_mood: Option<String>,
    /// Stage seen on the previous `update`. None until first call.
    last_stage: Option<String>,
    /// True until the first `update` call seeds the snapshot. Used to fire
    /// the hatch effect on the initial frame for a pet whose age == 0.
    first_update: bool,
    /// Milliseconds since the most recent enqueue.
    idle_ms: u32,
    /// True once at least one effect has been enqueued.
    has_run: bool,
    /// Duration of the longest currently active effect. Used by
    /// `has_active_effects` to decide when the animator is quiescent.
    active_duration_ms: u32,
    /// Wall-clock time of the most recent feed pulse detected in `update`.
    /// Exposed so `WatchApp` can forward it to `vm.last_feed_pulse_at`.
    pub last_feed_pulse_at: Option<time::OffsetDateTime>,
    /// Trigger IDs already seen this session, to prevent replays.
    seen_triggers: HashSet<crate::tui::room::SceneTriggerId>,
}

impl SceneAnimator {
    pub fn new() -> Self {
        Self {
            manager: EffectManager::default(),
            last_mood: None,
            last_stage: None,
            first_update: true,
            idle_ms: 0,
            has_run: false,
            active_duration_ms: 0,
            last_feed_pulse_at: None,
            seen_triggers: HashSet::new(),
        }
    }

    fn enqueue(&mut self, key: EffectKey, fx: Effect, area: Option<Rect>, duration_ms: u32) {
        let fx = if let Some(area) = area {
            fx.with_area(area)
        } else {
            fx
        };
        self.manager.add_unique_effect(key, fx);
        // If the animator was previously idle, start fresh with this effect's
        // duration. Otherwise extend the active window to the longest running
        // effect so the fast tick stays pinned correctly.
        if self.idle_ms >= self.active_duration_ms {
            self.active_duration_ms = duration_ms;
        } else {
            self.active_duration_ms = self.active_duration_ms.max(duration_ms);
        }
        self.idle_ms = 0;
        self.has_run = true;
    }

    /// Diff the view model against the previous frame's snapshot and enqueue
    /// effects for any detected transitions. Effects are flavored by species
    /// where it matters: glitch flickers harder on mood swings, mech sweeps
    /// in tighter on feed, etc.
    pub fn update(&mut self, vm: &WatchViewModel, targets: &SceneEffectTargets) {
        let species = Some(vm.pet_render.generated_species);

        // Hatch: first time we see a freshly-hatched pet, play coalesce.
        if self.first_update && vm.age_days == 0 {
            let duration = species_hatch_ms(species);
            self.enqueue(
                EffectKey::Hatch,
                coalesce(ms(duration)),
                Some(targets.pet),
                duration,
            );
        }

        // Stage-up: stage label changed. dissolve+coalesce sequence so the
        // pet morphs out and the new one fades in. Glitch dissolves longer
        // for a more chaotic rebirth; mech coalesces tighter.
        if let Some(prev) = &self.last_stage {
            if prev != &vm.stage {
                let (diss, coal) = species_stage_up_ms(species);
                let total = diss + coal;
                self.enqueue(
                    EffectKey::StageUp,
                    sequence(&[dissolve(ms(diss)), coalesce(ms(coal))]),
                    Some(targets.pet),
                    total,
                );
            }
        }

        // Mood fade: mood label changed. ~400ms hsl drift toward the new mood,
        // scaled by species (glitch swings harder; ghost barely registers).
        if let Some(prev) = &self.last_mood {
            if prev != &vm.mood {
                let drift = mood_hsl_drift_for(&vm.mood, species);
                self.enqueue(
                    EffectKey::MoodFade,
                    hsl_shift(Some(drift), None, ms(400)),
                    Some(targets.pet),
                    400,
                );
            }
        }

        // Feed pulse: the live profile reports the leading edge of a real
        // applied usage burst. This deliberately ignores daily aggregate jumps
        // so cold starts, backfills, and midnight rollover cannot fake a pop.
        let feed_pulse_at = if vm.life_profile.burst_level > 0.0 {
            vm.last_feed_pulse_at
        } else {
            None
        };
        if let Some(pulse_at) = feed_pulse_at {
            if self.last_feed_pulse_at != Some(pulse_at) {
                self.enqueue(
                    EffectKey::FeedPulse,
                    sweep_in(
                        Motion::LeftToRight,
                        10,
                        0,
                        species_feed_color(species),
                        ms(400),
                    ),
                    Some(targets.pet),
                    400,
                );
            }
        }
        self.last_feed_pulse_at = feed_pulse_at;

        self.last_mood = Some(vm.mood.clone());
        self.last_stage = Some(vm.stage.clone());
        self.first_update = false;

        let profile =
            crate::tui::room::derive_room_life_profile(vm, time::OffsetDateTime::now_utc());
        self.update_scene_moments(&profile.scene_moments, targets);
    }

    /// Enqueue effects for the supplied scene moments, using the provided
    /// target areas so each effect applies to the correct rect.
    pub fn update_scene_moments(
        &mut self,
        moments: &[crate::tui::room::SceneMoment],
        targets: &SceneEffectTargets,
    ) {
        for moment in moments {
            if !self.seen_triggers.insert(moment.trigger_id.clone()) {
                continue;
            }
            let base_key = SceneEffectKey {
                moment: moment.key,
                trigger_id: moment.trigger_id.clone(),
            };
            let duration = moment.duration_ms as u32;
            match moment.target_id {
                "watch.room.effect" => {
                    if targets.room_present {
                        if !targets.room_slices.is_empty() {
                            for (i, slice) in targets.room_slices.iter().enumerate() {
                                let key = EffectKey::Scene(SceneEffectKey {
                                    moment: moment.key,
                                    trigger_id: crate::tui::room::SceneTriggerId::new(format!(
                                        "{}:slice:{}",
                                        moment.trigger_id.as_str(),
                                        i
                                    )),
                                });
                                self.enqueue(
                                    key,
                                    effect_for_moment(moment),
                                    Some(*slice),
                                    duration,
                                );
                            }
                        }
                        // If room is present but slices is empty, the room is fully
                        // occluded — skip the effect rather than falling back to frame.
                    } else {
                        // Room target absent: fall back to the full frame.
                        let key = EffectKey::Scene(base_key);
                        self.enqueue(
                            key,
                            effect_for_moment(moment),
                            Some(targets.frame),
                            duration,
                        );
                    }
                }
                "watch.pet.effect" => {
                    self.enqueue(
                        EffectKey::Scene(base_key),
                        effect_for_moment(moment),
                        Some(targets.pet),
                        duration,
                    );
                }
                target_id => {
                    if let Some(rect) = targets.props.get(target_id) {
                        self.enqueue(
                            EffectKey::Scene(base_key),
                            effect_for_moment(moment),
                            Some(*rect),
                            duration,
                        );
                    }
                }
            }
        }
    }

    /// Render any active effects on top of `buf` within `targets.frame`. Called from
    /// the dispatcher after the watch frame has rendered its base content.
    /// `elapsed_ms` is the milliseconds since the previous apply.
    pub fn apply(&mut self, targets: &SceneEffectTargets, buf: &mut Buffer, elapsed_ms: u32) {
        self.manager
            .process_effects(ms(elapsed_ms), buf, targets.frame);
        self.idle_ms = self.idle_ms.saturating_add(elapsed_ms);
    }

    /// Whether any effect is currently running. True while `idle_ms` is less
    /// than the longest active effect duration. The watch loop uses this to
    /// pick the faster tick rate while transitions are in flight.
    pub fn has_active_effects(&self) -> bool {
        self.has_run && self.idle_ms < self.active_duration_ms
    }

    #[cfg(test)]
    pub fn advance_for_test(&mut self, ms: u32) {
        self.idle_ms = self.idle_ms.saturating_add(ms);
    }

    #[cfg(test)]
    pub fn active_until_ms_for_test(&self) -> u32 {
        self.active_duration_ms
    }
}

impl Default for SceneAnimator {
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
        "ecstatic" => [20.0, 25.0, 15.0], // warmer, brighter, more saturated than happy
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
    compute_breath_offset_with_rhythm(species, now, BreathRhythm::Awake)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreathRhythm {
    Awake,
    /// Slowed cycle whose phase is anchored at the sleep-onset instant so the
    /// period change is continuous, not a pop.
    Asleep {
        onset: time::OffsetDateTime,
    },
    /// Lengthened period for a tired-but-awake pet. eighths in 0..=8 maps to
    /// period scale 1.0..=TIRED_BREATH_MAX_SCALE (integer to keep Copy+Eq).
    Tired {
        eighths: u8,
    },
}

pub fn compute_breath_offset_with_rhythm(
    species: Option<Species>,
    now: time::OffsetDateTime,
    rhythm: BreathRhythm,
) -> u8 {
    let (period_ds, inhale_ds) = species_breath_rhythm_decis(species);
    let (period_ds, inhale_ds, anchor_ds) = match rhythm {
        BreathRhythm::Awake => (period_ds, inhale_ds, 0),
        BreathRhythm::Asleep { onset } => (
            period_ds * SLEEP_BREATH_PERIOD_SCALE,
            inhale_ds * SLEEP_BREATH_INHALE_SCALE,
            onset.unix_timestamp() * 10 + i64::from(onset.millisecond() / 100),
        ),
        BreathRhythm::Tired { eighths } => {
            let stretch = 16 + i64::from(eighths.min(8));
            (period_ds * stretch / 16, inhale_ds, 0)
        }
    };
    let ts_ds = now.unix_timestamp() * 10 + i64::from(now.millisecond() / 100);
    let phase = (ts_ds - anchor_ds).rem_euclid(period_ds);
    if phase < inhale_ds {
        1
    } else {
        0
    }
}

pub fn breath_rhythm_for_day(day: &crate::tui::day::DayContext) -> BreathRhythm {
    if let (true, Some(onset)) = (day.asleep, day.sleep_onset_utc) {
        return BreathRhythm::Asleep { onset };
    }
    if day.tiredness > TIRED_BREATH_MIN_TIREDNESS {
        return BreathRhythm::Tired {
            eighths: (day.tiredness.clamp(0.0, 1.0) * 8.0).round() as u8,
        };
    }
    BreathRhythm::Awake
}

/// Per-species breath rhythm in deciseconds (tenths-of-a-second).
/// Returns `(period, inhale_window)`.
///
/// Values are derived from the corresponding `breathPeriod` / `breathHold`
/// pair in `pet.jsx`, converted to deciseconds at 200 ms per decis
/// (multiply each by 2):
///
/// | Species | (period, inhale) | Cycle | Inhale |
/// |---------|------------------|-------|--------|
/// | Glitch  | (18, 4)          | 1.8 s | 0.4 s  |
/// | Ghost   | (22, 6)          | 2.2 s | 0.6 s  |
/// | Blob    | (26, 10)         | 2.6 s | 1.0 s  |
/// | Fuzz    | (32, 8)          | 3.2 s | 0.8 s  |
/// | Mech    | (34, 8)          | 3.4 s | 0.8 s  |
/// | Crystal | (38, 12)         | 3.8 s | 1.2 s  |
/// | None    | (32, 8)          | Fuzz-like default |
fn species_breath_rhythm_decis(species: Option<Species>) -> (i64, i64) {
    match species {
        Some(Species::Glitch) => (18, 4),
        Some(Species::Ghost) => (22, 6),
        Some(Species::Blob) => (26, 10),
        Some(Species::Fuzz) => (32, 8),
        Some(Species::Mech) => (34, 8),
        Some(Species::Crystal) => (38, 12),
        None => (32, 8),
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
///
/// Retained for tests that assert its behavior. Production code now uses
/// `compute_wander_position_x` computed directly in the panel renderer.
pub fn compute_wander_offset(now: time::OffsetDateTime) -> i8 {
    let step = now.unix_timestamp().rem_euclid(64) / 8;
    match step {
        2 => 1,
        6 => -1,
        _ => 0,
    }
}

/// Returns the pet's signed column offset from habitat center, clamped to
/// `[-half_range, +half_range]` where `half_range = (habitat_width - PET_W) / 2`.
/// Uses smoothstep interpolation between randomly-chosen target columns that
/// change every `TARGET_HOLD_SECS`, so the pet drifts edge-to-edge instead of
/// staying near center.
///
/// Deterministic function of `(habitat_width, species, now)` — no state needed.
pub fn compute_wander_position_x(
    habitat_width: u16,
    species: Species,
    now: time::OffsetDateTime,
) -> i16 {
    let half_range = (habitat_width.saturating_sub(PET_W) / 2) as i32;
    if half_range == 0 {
        return 0;
    }
    let ts = now.unix_timestamp();
    let period = ts / TARGET_HOLD_SECS;
    let t_in_period = (ts.rem_euclid(TARGET_HOLD_SECS)) as f32 / TARGET_HOLD_SECS as f32;
    let prev_target = wander_deterministic_target(period - 1, species, half_range);
    let curr_target = wander_deterministic_target(period, species, half_range);
    let ease = wander_ease_in_out(t_in_period);
    let position = prev_target as f32 * (1.0 - ease) + curr_target as f32 * ease;
    (position as i32).clamp(-half_range, half_range) as i16
}

/// Deterministic target column in `[-half_range, +half_range]` for a given
/// drift period. Uses a splitmix64-style hash of `(period, species_seed)`.
fn wander_deterministic_target(period: i64, species: Species, half_range: i32) -> i32 {
    let species_seed = match species {
        Species::Fuzz => 0u64,
        Species::Blob => 1,
        Species::Ghost => 2,
        Species::Glitch => 3,
        Species::Crystal => 4,
        Species::Mech => 5,
    };
    let h = splitmix64(period as u64 ^ species_seed.wrapping_mul(0x9E3779B97F4A7C15));
    // Map [0, u64::MAX] to [-half_range, +half_range].
    let range = (2 * half_range + 1) as u64;
    let bucket = h % range;
    bucket as i32 - half_range
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    x ^ (x >> 31)
}

fn wander_ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Wander position while asleep: blends from the live curve into the held
/// onset evaluation over WANDER_SETTLE_SECS. At the onset instant the two
/// endpoints coincide (now == onset), so sleep onset is continuous.
pub fn compute_sleep_wander_x(
    habitat_width: u16,
    species: Species,
    now: time::OffsetDateTime,
    onset: time::OffsetDateTime,
) -> i16 {
    let held = compute_wander_position_x(habitat_width, species, onset);
    let live = compute_wander_position_x(habitat_width, species, now);
    let k = ease_fraction(now, onset);
    blend_positions(live, held, k)
}

/// Wander position right after a wake: eases from the frozen sleep position
/// back onto the live curve over WANDER_SETTLE_SECS.
pub fn compute_wake_wander_x(
    habitat_width: u16,
    species: Species,
    now: time::OffsetDateTime,
    from_eval: time::OffsetDateTime,
    woke_at: time::OffsetDateTime,
) -> i16 {
    let frozen = compute_wander_position_x(habitat_width, species, from_eval);
    let live = compute_wander_position_x(habitat_width, species, now);
    let k = ease_fraction(now, woke_at);
    blend_positions(frozen, live, k)
}

/// Weekend-lazy wander clock: runs at 1/(1 + softening) speed, anchored at a
/// vm-carried instant so motion is continuous while softening holds. A
/// softening change (live activity beginning, the midnight weekend edge)
/// re-times the clock in one step — bounded, poll-aligned, and coincident
/// with the activity that caused it (live channels win; spec: Weekend
/// texture).
pub fn lazy_wander_instant(
    now: time::OffsetDateTime,
    anchor: time::OffsetDateTime,
    softening: f32,
) -> time::OffsetDateTime {
    let s = softening.clamp(0.0, 1.0);
    if s <= 0.0 {
        return now;
    }
    let elapsed = (now - anchor).as_seconds_f64().max(0.0);
    anchor + time::Duration::seconds_f64(elapsed / (1.0 + f64::from(s)))
}

fn ease_fraction(now: time::OffsetDateTime, since: time::OffsetDateTime) -> f32 {
    let elapsed_ms = (now - since).whole_milliseconds().max(0) as f32;
    (elapsed_ms / 1_000.0 / WANDER_SETTLE_SECS as f32).clamp(0.0, 1.0)
}

fn blend_positions(from: i16, to: i16, k: f32) -> i16 {
    (f32::from(from) * (1.0 - k) + f32::from(to) * k).round() as i16
}

/// Returns `+1` if the pet's current drift target is to the right of the
/// previous period's target, `-1` if to the left, or `+1` as a default when
/// there is no movement. Uses the same `half_range` as
/// `compute_wander_position_x` so the sign of position change and the facing
/// value are always consistent.
pub fn compute_facing(habitat_width: u16, species: Species, now: time::OffsetDateTime) -> i8 {
    let half_range = (habitat_width.saturating_sub(PET_W) / 2) as i32;
    if half_range == 0 {
        return 1;
    }
    let period = now.unix_timestamp() / TARGET_HOLD_SECS;
    let prev = wander_deterministic_target(period - 1, species, half_range);
    let curr = wander_deterministic_target(period, species, half_range);
    match (curr - prev).signum() {
        s if s > 0 => 1,
        s if s < 0 => -1,
        _ => 1, // no movement — default right-facing
    }
}

/// Returns `Some(role)` for ~1 second every 22 seconds, rotating through body,
/// accent, and pattern roles. The renderer brightens that role's foreground for
/// the shimmer window. Returns `None` outside the shimmer window.
pub fn compute_shimmer_role(
    species: Species,
    now: time::OffsetDateTime,
) -> Option<PaletteRoleName> {
    let pos = now.unix_timestamp().rem_euclid(SHIMMER_PERIOD_SECS);
    if pos >= SHIMMER_DURATION_SECS {
        return None;
    }
    let period = now.unix_timestamp() / SHIMMER_PERIOD_SECS;
    let roles = [
        PaletteRoleName::Body,
        PaletteRoleName::Accent,
        PaletteRoleName::Pattern,
    ];
    let idx = splitmix64(period as u64 ^ species as u64) as usize % roles.len();
    Some(roles[idx])
}

/// A sparkle glyph overlaid on a single pet-art cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TwinkleSpec {
    /// Row inside the 11×8 art grid (0-based).
    pub row: u8,
    /// Column inside the 11×8 art grid (0-based).
    pub col: u8,
    pub glyph: char,
}

/// Returns a twinkle spec for ~1 frame every `TWINKLE_PERIOD_SECS` seconds.
/// Position and glyph are deterministic per `(species, period)`.
pub fn compute_twinkle(species: Species, now: time::OffsetDateTime) -> Option<TwinkleSpec> {
    // Only active during the first half-second of the period.
    let ts_ds = now.unix_timestamp() * 10 + i64::from(now.millisecond() / 100);
    let period_ds = TWINKLE_PERIOD_SECS * 10;
    let pos_ds = ts_ds.rem_euclid(period_ds);
    if pos_ds >= 5 {
        return None;
    }
    let period = now.unix_timestamp() / TWINKLE_PERIOD_SECS;
    let h = splitmix64(period as u64 ^ species as u64 ^ 0xDEADBEEF_CAFEBABE);
    let row = (h % 8) as u8;
    let col = ((h >> 8) % 11) as u8;
    let glyph = twinkle_glyph_for(species, (h >> 16) as usize);
    Some(TwinkleSpec { row, col, glyph })
}

fn twinkle_glyph_for(species: Species, idx: usize) -> char {
    let glyphs: &[char] = match species {
        Species::Glitch => &['\u{2592}', '\u{2591}', '\u{2593}', '*'],
        _ => &['\u{2726}', '\u{2727}', '*', '\u{b7}'],
    };
    glyphs[idx % glyphs.len()]
}

/// Brief visual pop that fires for 2 seconds after a feed pulse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenPop {
    /// How many whole seconds remain in the 2-second pop window.
    pub duration_remaining_secs: i64,
}

/// Returns `Some(TokenPop)` within 2 seconds of `last_feed_pulse_at`, `None`
/// before or after that window.
pub fn compute_token_pop(
    last_feed_pulse_at: Option<time::OffsetDateTime>,
    now: time::OffsetDateTime,
) -> Option<TokenPop> {
    let pulse = last_feed_pulse_at?;
    let elapsed = (now - pulse).whole_seconds();
    if !(0..=2).contains(&elapsed) {
        return None;
    }
    Some(TokenPop {
        duration_remaining_secs: 2 - elapsed,
    })
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

/// Build a tachyonfx effect for the given scene moment.
fn effect_for_moment(moment: &crate::tui::room::SceneMoment) -> Effect {
    use crate::tui::room::SceneMomentKey;
    match moment.key {
        SceneMomentKey::FeedSweep => sweep_in(
            Motion::LeftToRight,
            10,
            0,
            Color::Yellow,
            ms(moment.duration_ms as u32),
        ),
        SceneMomentKey::PropResonanceRipple => coalesce(ms(moment.duration_ms as u32)),
        SceneMomentKey::DawnWakeWipe => sweep_in(
            Motion::LeftToRight,
            10,
            0,
            Color::Cyan,
            ms(moment.duration_ms as u32),
        ),
        SceneMomentKey::HeavySessionShimmer => {
            hsl_shift(Some([0.0, 20.0, 10.0]), None, ms(moment.duration_ms as u32))
        }
        SceneMomentKey::DreamGlimmer => coalesce(ms(moment.duration_ms as u32)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_vm() -> WatchViewModel {
        WatchViewModel::fixture()
    }

    fn dummy_targets() -> SceneEffectTargets {
        SceneEffectTargets {
            frame: Rect::new(0, 0, 80, 24),
            pet: Rect::new(10, 5, 20, 10),
            room_present: false,
            room_slices: vec![],
            props: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn new_animator_has_no_active_effects() {
        let a = SceneAnimator::new();
        assert!(!a.has_active_effects());
    }

    #[test]
    fn first_update_for_age_zero_pet_enqueues_hatch() {
        let mut a = SceneAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 0;
        a.update(&vm, &dummy_targets());
        assert!(a.has_active_effects(), "hatch should be queued");
    }

    #[test]
    fn first_update_for_older_pet_does_not_hatch() {
        let mut a = SceneAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 5;
        a.update(&vm, &dummy_targets());
        assert!(!a.has_active_effects(), "no hatch for an older pet");
    }

    #[test]
    fn mood_change_enqueues_fade() {
        let mut a = SceneAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 5; // avoid hatch
        vm.mood = "content".into();
        a.update(&vm, &dummy_targets());
        assert!(!a.has_active_effects(), "no effects after initial snapshot");

        vm.mood = "sleepy".into();
        a.update(&vm, &dummy_targets());
        assert!(a.has_active_effects(), "mood change should fire fade");
    }

    #[test]
    fn stage_change_enqueues_stage_up() {
        let mut a = SceneAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 5;
        vm.stage = "hatchling".into();
        a.update(&vm, &dummy_targets());
        vm.stage = "juvenile".into();
        a.update(&vm, &dummy_targets());
        assert!(a.has_active_effects(), "stage change should fire stage-up");
    }

    #[test]
    fn feed_pulse_ignores_daily_token_growth_without_profile_burst() {
        let mut a = SceneAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 5;
        vm.today_effective_tokens = 1_000.0;
        vm.life_profile.burst_level = 0.0;
        a.update(&vm, &dummy_targets());

        vm.today_effective_tokens = 5_000.0;
        vm.life_profile.burst_level = 0.0;
        a.update(&vm, &dummy_targets());

        assert!(
            !a.has_active_effects(),
            "daily total growth should not bypass the live profile burst contract"
        );
        assert_eq!(a.last_feed_pulse_at, None);
    }

    #[test]
    fn feed_pulse_fires_on_profile_burst() {
        let mut a = SceneAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 5;
        vm.today_effective_tokens = 1_000.0;
        vm.life_profile.burst_level = 0.0;
        a.update(&vm, &dummy_targets());

        vm.life_profile.burst_level = 0.8;
        vm.last_feed_pulse_at = Some(time::OffsetDateTime::from_unix_timestamp(1_000).unwrap());
        a.update(&vm, &dummy_targets());

        assert!(a.has_active_effects(), "profile burst should pulse");
        assert!(a.last_feed_pulse_at.is_some());
    }

    #[test]
    fn feed_pulse_fires_on_first_seen_profile_burst_timestamp() {
        let mut a = SceneAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 5;
        vm.life_profile.burst_level = 0.8;
        vm.last_feed_pulse_at = Some(time::OffsetDateTime::from_unix_timestamp(1_000).unwrap());

        a.update(&vm, &dummy_targets());

        assert!(
            a.has_active_effects(),
            "first seen profile burst should pulse"
        );
        assert_eq!(a.last_feed_pulse_at, vm.last_feed_pulse_at);
    }

    #[test]
    fn feed_pulse_refreshes_for_new_profile_burst_timestamp() {
        let mut a = SceneAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 5;
        vm.life_profile.burst_level = 0.8;
        vm.last_feed_pulse_at = Some(time::OffsetDateTime::from_unix_timestamp(1_000).unwrap());
        a.update(&vm, &dummy_targets());
        a.idle_ms = a.active_duration_ms;

        vm.last_feed_pulse_at = Some(time::OffsetDateTime::from_unix_timestamp(1_010).unwrap());
        a.update(&vm, &dummy_targets());

        assert_eq!(a.last_feed_pulse_at, vm.last_feed_pulse_at);
        assert_eq!(a.idle_ms, 0, "new poll burst should refresh the pulse");
    }

    #[test]
    fn small_token_growth_does_not_fire_feed_pulse() {
        let mut a = SceneAnimator::new();
        let mut vm = fixture_vm();
        vm.age_days = 5;
        vm.today_effective_tokens = 1_000.0;
        a.update(&vm, &dummy_targets());
        vm.today_effective_tokens = 1_050.0; // +50, below threshold
        a.update(&vm, &dummy_targets());
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
    fn breath_periods_match_pet_jsx_ordering() {
        // pet.jsx SPECIES_ANIM breathPeriod: glitch 9 < ghost 11 < blob 13 < fuzz 16 < mech 17 < crystal 19.
        let p = |s| species_breath_rhythm_decis(Some(s)).0;
        assert!(p(Species::Glitch) < p(Species::Ghost));
        assert!(p(Species::Ghost) < p(Species::Blob));
        assert!(p(Species::Blob) < p(Species::Fuzz));
        assert!(p(Species::Fuzz) <= p(Species::Mech));
        assert!(p(Species::Mech) < p(Species::Crystal));
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

    // ── compute_wander_position_x ─────────────────────────────────────────

    #[test]
    fn wander_position_returns_zero_when_habitat_too_narrow() {
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        // habitat_width == PET_W: no room to drift.
        assert_eq!(compute_wander_position_x(PET_W, Species::Fuzz, now), 0);
        // habitat_width < PET_W: saturating_sub gives 0.
        assert_eq!(compute_wander_position_x(0, Species::Fuzz, now), 0);
    }

    #[test]
    fn wander_position_stays_within_half_range() {
        use time::macros::datetime;
        let start = datetime!(2026-05-11 12:00:00 UTC);
        let habitat_width: u16 = 52;
        let half_range = ((habitat_width - PET_W) / 2) as i16;
        for s in 0..=TARGET_HOLD_SECS * 4 {
            let now = start + time::Duration::seconds(s);
            let pos = compute_wander_position_x(habitat_width, Species::Fuzz, now);
            assert!(
                pos >= -half_range && pos <= half_range,
                "pos {pos} outside [-{half_range}, {half_range}] at s={s}"
            );
        }
    }

    #[test]
    fn wander_position_is_deterministic() {
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_042).unwrap();
        let a = compute_wander_position_x(52, Species::Crystal, now);
        let b = compute_wander_position_x(52, Species::Crystal, now);
        assert_eq!(a, b, "same inputs must give same output");
    }

    #[test]
    fn wander_position_starts_near_prev_target_and_ends_near_curr_target() {
        // Pick a timestamp at the exact start of a period (ts divisible by TARGET_HOLD_SECS).
        // At t=0 within a period: ease = 0.0, position = prev.
        // At t≈1 within a period: ease ≈ 1.0, position ≈ curr.
        let period_start_ts = 1_700_000_000i64 - (1_700_000_000i64 % TARGET_HOLD_SECS);
        let base = time::OffsetDateTime::from_unix_timestamp(period_start_ts).unwrap();
        let period = base.unix_timestamp() / TARGET_HOLD_SECS;
        let half_range = (52 - PET_W as i32) / 2;
        let prev = wander_deterministic_target(period - 1, Species::Blob, half_range);
        let curr = wander_deterministic_target(period, Species::Blob, half_range);

        // At t=0 the position equals prev exactly.
        let at_start = compute_wander_position_x(52, Species::Blob, base);
        assert_eq!(
            at_start as i32, prev,
            "at t=0 position should equal prev {prev}"
        );

        // Near end of period: position should be closer to curr than prev.
        // Only meaningful when prev != curr.
        if prev != curr {
            let near_end = base + time::Duration::seconds(TARGET_HOLD_SECS - 1);
            let at_end = compute_wander_position_x(52, Species::Blob, near_end);
            assert!(
                (at_end as i32 - curr).abs() <= (at_end as i32 - prev).abs(),
                "at_end {at_end} should be closer to curr {curr} than prev {prev}"
            );
        }
    }

    // ── compute_facing ────────────────────────────────────────────────────

    #[test]
    fn facing_returns_plus_or_minus_one() {
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        for species in Species::all() {
            let f = compute_facing(120, species, now);
            assert!(f == 1 || f == -1, "got {f} for {species:?}");
        }
    }

    #[test]
    fn facing_is_deterministic() {
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let a = compute_facing(120, Species::Ghost, now);
        let b = compute_facing(120, Species::Ghost, now);
        assert_eq!(a, b);
    }

    #[test]
    fn facing_matches_actual_drift_direction() {
        // For each species and each of several periods, the sign of
        // (wander_position_x at period_end) - (wander_position_x at prev_period_end)
        // should match compute_facing's output for that period.
        let habitat_width = 120u16;
        for species in [
            Species::Fuzz,
            Species::Blob,
            Species::Ghost,
            Species::Glitch,
            Species::Crystal,
            Species::Mech,
        ] {
            for period_index in 0..20i64 {
                let base_ts = period_index * TARGET_HOLD_SECS;
                let end_of_prev_period =
                    time::OffsetDateTime::from_unix_timestamp(base_ts - 1).unwrap();
                let end_of_curr_period =
                    time::OffsetDateTime::from_unix_timestamp(base_ts + TARGET_HOLD_SECS - 1)
                        .unwrap();
                let pos_prev =
                    compute_wander_position_x(habitat_width, species, end_of_prev_period);
                let pos_curr =
                    compute_wander_position_x(habitat_width, species, end_of_curr_period);
                let actual_sign = (pos_curr as i32 - pos_prev as i32).signum();
                let claimed_facing = compute_facing(habitat_width, species, end_of_curr_period);
                if actual_sign != 0 {
                    let expected = if actual_sign > 0 { 1 } else { -1 };
                    assert_eq!(
                        claimed_facing,
                        expected,
                        "species {species:?} period {period_index}: pos {pos_prev}→{pos_curr} but facing says {claimed_facing}"
                    );
                }
            }
        }
    }

    // ── compute_shimmer_role ──────────────────────────────────────────────

    #[test]
    fn shimmer_is_none_outside_window() {
        use time::macros::datetime;
        let base = datetime!(2026-05-11 12:00:00 UTC);
        // After the first second, shimmer should be off for most of the period.
        let mut none_count = 0;
        for s in 0..(SHIMMER_PERIOD_SECS * 2) {
            let now = base + time::Duration::seconds(s);
            if compute_shimmer_role(Species::Fuzz, now).is_none() {
                none_count += 1;
            }
        }
        // At most SHIMMER_DURATION_SECS per SHIMMER_PERIOD_SECS should be Some.
        let total = SHIMMER_PERIOD_SECS * 2;
        let max_some = SHIMMER_DURATION_SECS * 2 + 1;
        assert!(
            none_count >= total - max_some,
            "shimmer should be None most of the time, was None {none_count}/{total}"
        );
    }

    #[test]
    fn shimmer_is_some_at_period_start() {
        // Timestamp 0 is at the start of a period for any SHIMMER_PERIOD_SECS.
        let now = time::OffsetDateTime::from_unix_timestamp(0).unwrap();
        assert!(
            compute_shimmer_role(Species::Crystal, now).is_some(),
            "shimmer should fire at period start"
        );
    }

    #[test]
    fn shimmer_role_is_body_accent_or_pattern() {
        let now = time::OffsetDateTime::from_unix_timestamp(0).unwrap();
        let role = compute_shimmer_role(Species::Mech, now).unwrap();
        assert!(
            matches!(
                role,
                PaletteRoleName::Body | PaletteRoleName::Accent | PaletteRoleName::Pattern
            ),
            "shimmer role must be Body, Accent, or Pattern, got {role:?}"
        );
    }

    // ── compute_twinkle ───────────────────────────────────────────────────

    #[test]
    fn twinkle_position_within_art_bounds() {
        let now = time::OffsetDateTime::from_unix_timestamp(0).unwrap();
        if let Some(t) = compute_twinkle(Species::Crystal, now) {
            assert!(t.row < 8, "twinkle row {} out of art bounds", t.row);
            assert!(t.col < 11, "twinkle col {} out of art bounds", t.col);
        }
    }

    #[test]
    fn twinkle_fires_at_period_start() {
        let now = time::OffsetDateTime::from_unix_timestamp(0).unwrap();
        assert!(
            compute_twinkle(Species::Fuzz, now).is_some(),
            "twinkle should fire at period start (unix ts 0)"
        );
    }

    #[test]
    fn twinkle_is_none_outside_window() {
        // Milliseconds 500+ in the period should be None.
        let now = time::OffsetDateTime::from_unix_timestamp(1).unwrap();
        // 1 second into a 7-second period, > 500ms into the period.
        assert!(
            compute_twinkle(Species::Fuzz, now).is_none(),
            "twinkle should be None outside the half-second window"
        );
    }

    // ── compute_token_pop ─────────────────────────────────────────────────

    #[test]
    fn token_pop_none_when_no_pulse() {
        let now = time::OffsetDateTime::from_unix_timestamp(1_000).unwrap();
        assert!(compute_token_pop(None, now).is_none());
    }

    #[test]
    fn token_pop_some_within_two_seconds() {
        let now = time::OffsetDateTime::from_unix_timestamp(1_000).unwrap();
        let pulse = now - time::Duration::seconds(1);
        let pop = compute_token_pop(Some(pulse), now);
        assert!(pop.is_some(), "should be Some within 2s of pulse");
        assert_eq!(pop.unwrap().duration_remaining_secs, 1);
    }

    #[test]
    fn token_pop_none_after_two_seconds() {
        let now = time::OffsetDateTime::from_unix_timestamp(1_000).unwrap();
        let pulse = now - time::Duration::seconds(3);
        assert!(compute_token_pop(Some(pulse), now).is_none());
    }

    #[test]
    fn token_pop_none_before_pulse() {
        let now = time::OffsetDateTime::from_unix_timestamp(1_000).unwrap();
        let pulse = now + time::Duration::seconds(1);
        assert!(compute_token_pop(Some(pulse), now).is_none());
    }

    #[test]
    fn low_energy_multiplier_clamps() {
        assert_eq!(low_energy_lightness_multiplier(0.7), 1.0);
        assert_eq!(low_energy_lightness_multiplier(0.6), 1.0);
        let mid = low_energy_lightness_multiplier(0.3);
        assert!(mid > 0.55 && mid < 1.0, "got {mid}");
        assert_eq!(low_energy_lightness_multiplier(0.0), 0.55);
    }

    #[test]
    fn sleep_breath_is_slower_and_continuous_at_onset() {
        let onset = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        // At the onset instant, the asleep phase starts at zero — identical
        // breath state to a fresh awake cycle start (no visible pop).
        let at_onset = compute_breath_offset_with_rhythm(
            Some(Species::Fuzz),
            onset,
            BreathRhythm::Asleep { onset },
        );
        assert_eq!(at_onset, 1, "phase 0 sits inside the inhale window");
        // The asleep cycle is SLEEP_BREATH_PERIOD_SCALE x longer: for fuzz
        // (period 32ds, inhale 8ds) the asleep inhale window is 16ds of a
        // 96ds cycle — at +5s (50ds) the pet must be at rest, and by +10s
        // (100ds) the next inhale is active.
        let mid = compute_breath_offset_with_rhythm(
            Some(Species::Fuzz),
            onset + time::Duration::seconds(5),
            BreathRhythm::Asleep { onset },
        );
        assert_eq!(mid, 0);
        let next_cycle = compute_breath_offset_with_rhythm(
            Some(Species::Fuzz),
            onset + time::Duration::seconds(10),
            BreathRhythm::Asleep { onset },
        );
        assert_eq!(next_cycle, 1);
    }

    #[test]
    fn awake_rhythm_matches_the_existing_breath_function() {
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_123).unwrap();
        for species in [Species::Fuzz, Species::Glitch, Species::Crystal] {
            assert_eq!(
                compute_breath_offset_with_rhythm(Some(species), now, BreathRhythm::Awake),
                compute_breath_offset(Some(species), now),
            );
        }
    }

    #[test]
    fn wander_holds_at_sleep_onset_and_eases_back_on_wake() {
        let onset = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let width = 52_u16;
        let held = compute_wander_position_x(width, Species::Fuzz, onset);
        // Deep into sleep, position is fully held at the onset evaluation.
        let deep = onset + time::Duration::minutes(40);
        assert_eq!(
            compute_sleep_wander_x(width, Species::Fuzz, deep, onset),
            held,
            "asleep wander must hold the onset position"
        );
        // At the onset instant itself the blend starts from the live curve,
        // which equals the held value — continuous by construction.
        assert_eq!(
            compute_sleep_wander_x(width, Species::Fuzz, onset, onset),
            compute_wander_position_x(width, Species::Fuzz, onset),
        );
        // Wake resume: at the wake instant the position is the frozen eval;
        // after WANDER_SETTLE_SECS it equals the live curve.
        let woke_at = deep;
        let from_eval = onset;
        assert_eq!(
            compute_wake_wander_x(width, Species::Fuzz, woke_at, from_eval, woke_at),
            compute_wander_position_x(width, Species::Fuzz, from_eval),
        );
        let settled = woke_at + time::Duration::seconds(WANDER_SETTLE_SECS);
        assert_eq!(
            compute_wake_wander_x(width, Species::Fuzz, settled, from_eval, woke_at),
            compute_wander_position_x(width, Species::Fuzz, settled),
        );
    }

    #[test]
    fn tired_breath_period_scale_at_full_eighths_equals_tired_breath_max_scale() {
        let species = Some(Species::Crystal);
        let rising_edges = |rhythm: BreathRhythm| {
            let base = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
            let mut prev = 0;
            let mut edges = 0;
            // 1800 deciseconds = 180 seconds = 3-minute observation window.
            for ds in 0..1800_i64 {
                let now = base + time::Duration::milliseconds(ds * 100);
                let cur = compute_breath_offset_with_rhythm(species, now, rhythm);
                if prev == 0 && cur == 1 {
                    edges += 1;
                }
                prev = cur;
            }
            edges
        };
        let awake = rising_edges(BreathRhythm::Awake);
        let tired = rising_edges(BreathRhythm::Tired { eighths: 8 });
        assert_eq!(awake, 48);
        assert_eq!(tired, 32);
        assert!(
            (f64::from(awake) / f64::from(tired) - TIRED_BREATH_MAX_SCALE).abs() < f64::EPSILON,
            "full-eighths period stretch must equal TIRED_BREATH_MAX_SCALE"
        );
    }

    #[test]
    fn breath_rhythm_for_day_picks_asleep_over_tired_over_awake() {
        let onset = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let asleep = crate::tui::day::DayContext {
            asleep: true,
            sleep_onset_utc: Some(onset),
            tiredness: 1.0,
            ..Default::default()
        };
        assert_eq!(
            breath_rhythm_for_day(&asleep),
            BreathRhythm::Asleep { onset }
        );
        let tired = crate::tui::day::DayContext {
            tiredness: 0.5,
            ..Default::default()
        };
        assert_eq!(
            breath_rhythm_for_day(&tired),
            BreathRhythm::Tired { eighths: 4 }
        );
        let barely = crate::tui::day::DayContext {
            tiredness: 0.05,
            ..Default::default()
        };
        assert_eq!(breath_rhythm_for_day(&barely), BreathRhythm::Awake);
        assert_eq!(
            breath_rhythm_for_day(&crate::tui::day::DayContext::default()),
            BreathRhythm::Awake
        );
    }

    #[test]
    fn lazy_wander_instant_dilates_elapsed_time_only_when_softened() {
        let anchor = time::OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap();
        let now = anchor + time::Duration::minutes(60);
        assert_eq!(lazy_wander_instant(now, anchor, 0.0), now);
        assert_eq!(
            lazy_wander_instant(now, anchor, 1.0),
            anchor + time::Duration::minutes(30)
        );
        assert_eq!(
            lazy_wander_instant(anchor - time::Duration::minutes(5), anchor, 1.0),
            anchor
        );
    }

    // ── scene moment tests ────────────────────────────────────────────────

    #[test]
    fn scene_moment_triggers_only_once_per_trigger_id() {
        let mut animator = SceneAnimator::new();
        let _vm = WatchViewModel::fixture();
        let moment = crate::tui::room::SceneMoment {
            key: crate::tui::room::SceneMomentKey::DawnWakeWipe,
            trigger_id: crate::tui::room::SceneTriggerId::new("wake:1"),
            target_id: "watch.room.effect",
            duration_ms: 900,
            max_replay_age_ms: 3_600_000,
        };

        animator.update_scene_moments(std::slice::from_ref(&moment), &dummy_targets());
        assert!(animator.has_active_effects());
        let first_active_until = animator.active_until_ms_for_test();

        animator.update_scene_moments(&[moment], &dummy_targets());

        assert_eq!(animator.active_until_ms_for_test(), first_active_until);
    }

    #[test]
    fn scene_moment_expiry_controls_active_effects() {
        let mut animator = SceneAnimator::new();
        let _vm = WatchViewModel::fixture();
        let moment = crate::tui::room::SceneMoment {
            key: crate::tui::room::SceneMomentKey::PropResonanceRipple,
            trigger_id: crate::tui::room::SceneTriggerId::new("prop:1"),
            target_id: "watch.room.effect",
            duration_ms: 700,
            max_replay_age_ms: 3_600_000,
        };

        animator.update_scene_moments(&[moment], &dummy_targets());
        animator.advance_for_test(699);
        assert!(animator.has_active_effects());
        animator.advance_for_test(1);
        assert!(!animator.has_active_effects());
    }
}
