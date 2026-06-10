//! Day phase and rhythm derivation for the time-of-day presentation layer.
//!
//! `DayContext` itself is built in Task 6; this module provides the
//! phase-window constants and algorithm it consumes.
//!
//! See docs/superpowers/specs/2026-06-09-glorp-lives-in-time-design.md.
//! Everything here is a pure function of (clock, mapper, ledger aggregates);
//! nothing is persisted.

use crate::storage::day_axis::LocalDayMapper;
use crate::storage::state::PetState;
use crate::storage::usage_store::{AppliedShapeSums, UsageStore};
use crate::tui::life::{classify_work_weather, TokenShapeDelta, WorkWeather};
use std::collections::{HashMap, HashSet};
use time::{Duration, PrimitiveDateTime, Time};

/// Trailing window for the activity-rhythm histogram, in local days.
pub const RHYTHM_WINDOW_DAYS: usize = 30;
/// An hour is "quiet" when its share of window volume is below this.
pub const RHYTHM_QUIET_SHARE: f64 = 0.01;
/// Quiet runs shorter than this can't be a night — fall back to defaults.
pub const MIN_NIGHT_RUN_HOURS: usize = 5;
/// Quiet runs longer than this are clamped (a 4h/day user must keep a Day).
pub const MAX_NIGHT_RUN_HOURS: usize = 12;
/// Dawn/Dusk shoulders carved from the quiet window's edges.
pub const PHASE_SHOULDER_HOURS: u8 = 2;
/// Personalization needs at least this many distinct active local days...
pub const MIN_ACTIVE_DAYS: usize = 5;
/// ...and this many distinct active hours (hour diversity).
pub const MIN_DISTINCT_ACTIVE_HOURS: usize = 3;
/// The pet sleeps only after this many minutes of night-phase ledger quiet,
/// and re-arms on the same window after a wake (symmetric by construction).
pub const SLEEP_IDLE_MINUTES: i64 = 20;
/// Phase palettes interpolate over this window after a phase boundary.
pub const PHASE_BLEND_MINUTES: i64 = 30;
/// Motes fade out over this window after local-day rollover instead of
/// vanishing mid-grind at 00:00 (spec: Boundary behavior).
pub const MOTE_TIDY_FADE_MINUTES: i64 = 30;
/// A finished day reads as a feast when its ratio clears this multiple
/// of the calibration baseline.
pub const FEAST_DAY_RATIO: f32 = 1.5;
/// Trailing window for accumulated-active-time fatigue, in hours.
pub const FATIGUE_WINDOW_HOURS: i64 = 16;
/// Morning-after flavor covers all of Dawn plus this many minutes of Day.
pub const MORNING_AFTER_DAY_MINUTES: i64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayPhase {
    Dawn,
    Day,
    Dusk,
    Night,
}

/// Local-hour starts of each phase, circular. dusk_start..night_start = Dusk,
/// night_start..dawn_start = Night, dawn_start..day_start = Dawn, rest = Day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseWindows {
    pub dusk_start: u8,
    pub night_start: u8,
    pub dawn_start: u8,
    pub day_start: u8,
}

impl PhaseWindows {
    /// Hand-set defaults until the ledger is mature: dawn 07-09, day 09-18,
    /// dusk 18-22, night 22-07.
    pub fn clock_defaults() -> Self {
        Self {
            dusk_start: 18,
            night_start: 22,
            dawn_start: 7,
            day_start: 9,
        }
    }

    pub fn phase_for_hour(&self, hour: u8) -> DayPhase {
        let h = hour % 24;
        if in_circular_range(h, self.dusk_start, self.night_start) {
            DayPhase::Dusk
        } else if in_circular_range(h, self.night_start, self.dawn_start) {
            DayPhase::Night
        } else if in_circular_range(h, self.dawn_start, self.day_start) {
            DayPhase::Dawn
        } else {
            DayPhase::Day
        }
    }
}

/// Half-open circular hour range test: start <= h < end, wrapping midnight.
fn in_circular_range(h: u8, start: u8, end: u8) -> bool {
    if start <= end {
        (start..end).contains(&h)
    } else {
        h >= start || h < end
    }
}

/// Derive phase windows from a local-hour volume histogram. Returns clock
/// defaults when the histogram has no usable quiet run (empty, all-active,
/// too-short run, or an ambiguous tie). See spec "Activity rhythm".
///
/// `histogram` must contain finite, non-negative volumes.
pub fn derive_phase_windows(histogram: &[f64; 24]) -> PhaseWindows {
    let total: f64 = histogram.iter().sum();
    if total <= 0.0 {
        return PhaseWindows::clock_defaults();
    }
    let quiet: Vec<bool> = histogram
        .iter()
        .map(|&v| v / total < RHYTHM_QUIET_SHARE)
        .collect();

    // Longest contiguous circular quiet run; ties are ambiguous.
    let mut best_start = 0_usize;
    let mut best_len = 0_usize;
    let mut tie = false;
    for start in 0..24 {
        if !quiet[start] || quiet[(start + 23) % 24] {
            continue; // only run heads (previous hour active)
        }
        let mut len = 0;
        while len < 24 && quiet[(start + len) % 24] {
            len += 1;
        }
        if len > best_len {
            best_len = len;
            best_start = start;
            tie = false;
        } else if len == best_len && len > 0 {
            tie = true;
        }
    }
    // All 24 quiet (no run head found) or nothing quiet:
    if best_len == 0 || tie {
        return PhaseWindows::clock_defaults();
    }
    if best_len < MIN_NIGHT_RUN_HOURS {
        return PhaseWindows::clock_defaults();
    }

    // Clamp over-long quiet runs to MAX, centered on the run midpoint.
    let (q_start, q_len) = if best_len > MAX_NIGHT_RUN_HOURS {
        let midpoint = (best_start + best_len / 2) % 24;
        let clamped_start = (midpoint + 24 - MAX_NIGHT_RUN_HOURS / 2) % 24;
        (clamped_start, MAX_NIGHT_RUN_HOURS)
    } else {
        (best_start, best_len)
    };

    let shoulder = PHASE_SHOULDER_HOURS as usize;
    PhaseWindows {
        dusk_start: (q_start % 24) as u8,
        night_start: ((q_start + shoulder) % 24) as u8,
        dawn_start: ((q_start + q_len - shoulder) % 24) as u8,
        day_start: ((q_start + q_len) % 24) as u8,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

/// One prior local day's observed character. `ratio` is vs the calibration
/// baseline; `dominant_shape` is None when the day had no token-shape detail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DaySummary {
    pub ratio: f32,
    pub dominant_shape: Option<WorkWeather>,
}

/// Wake-resume easing inputs for the wander hold (panel-side, pure).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WakeResume {
    /// Evaluate the frozen wander position at this instant...
    pub from_eval_utc: time::OffsetDateTime,
    /// ...and ease toward the live curve starting here.
    pub woke_at_utc: time::OffsetDateTime,
}

/// Derived time-of-day presentation contract. Built once per poll in
/// build_watch_view_model_at; per-frame logic only compares clock.now_utc()
/// against the precomputed UTC instants carried here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DayContext {
    pub day_phase: DayPhase,
    pub phase_started_at_utc: time::OffsetDateTime,
    pub phase_ends_at_utc: time::OffsetDateTime,
    /// hash(local date of the most recent Dawn entry, pet seed) — visual
    /// texture only, never personality content (locked rule).
    pub date_seed: u64,
    pub today_ratio: f32,
    pub yesterday: Option<DaySummary>,
    pub climate: Option<WorkWeather>,
    pub is_weekend: bool,
    pub weekend_share: f32,
    pub season: Season,
    /// Rhythm/baseline personalization gate (spec: Maturity gate).
    pub mature: bool,
    pub asleep: bool,
    pub sleep_onset_utc: Option<time::OffsetDateTime>,
    pub wake_resume: Option<WakeResume>,
    /// Next local-day rollover (T2 motes tidy fade).
    pub local_day_rollover_utc: time::OffsetDateTime,
    /// Accumulated-active-time fatigue, 0.0..=1.0. Derived from the count of
    /// 10-minute buckets containing applied tokens in the trailing
    /// FATIGUE_WINDOW_HOURS, scaled by the window's volume ratio vs baseline.
    /// 0.0 while the maturity gate is closed.
    pub tiredness: f32,
    /// UTC instant the current local day began (motes tidy-fade anchor).
    pub local_day_started_utc: time::OffsetDateTime,
}

impl Default for DayContext {
    /// Neutral daytime context for fixtures and pre-first-poll frames.
    fn default() -> Self {
        let epoch = time::OffsetDateTime::UNIX_EPOCH;
        Self {
            day_phase: DayPhase::Day,
            phase_started_at_utc: epoch,
            phase_ends_at_utc: epoch,
            date_seed: 0,
            today_ratio: 0.0,
            yesterday: None,
            climate: None,
            is_weekend: false,
            weekend_share: 0.0,
            season: Season::Summer,
            mature: false,
            asleep: false,
            sleep_onset_utc: None,
            wake_resume: None,
            local_day_rollover_utc: epoch,
            tiredness: 0.0,
            local_day_started_utc: epoch,
        }
    }
}

/// Build the DayContext from the applied ledger. Errors in any single read
/// degrade that field to its quiet default — DayContext must never take the
/// watch loop down.
pub fn build_day_context(
    usage_store: &UsageStore,
    state: &PetState,
    now: time::OffsetDateTime,
    mapper: LocalDayMapper,
) -> DayContext {
    let today = mapper.local_date(now);
    let now_local = now.to_offset(mapper.offset_at(now));

    // --- rhythm window: one aggregated query powers histogram + maturity +
    // weekend share ---
    let rhythm_starts = mapper.day_starts_back(now, RHYTHM_WINDOW_DAYS);
    let bucket_sums = usage_store
        .applied_bucket_sums_between(rhythm_starts[0], *rhythm_starts.last().unwrap())
        .unwrap_or_default();
    let mut histogram = [0.0_f64; 24];
    let mut active_days: HashSet<time::Date> = HashSet::new();
    let mut active_hours: HashSet<u8> = HashSet::new();
    let mut weekend_volume = 0.0_f64;
    let mut total_volume = 0.0_f64;
    for &(bucket_at, tokens) in &bucket_sums {
        if tokens <= 0.0 {
            continue;
        }
        let local = bucket_at.to_offset(mapper.offset_at(bucket_at));
        let hour = local.hour();
        let date = local.date();
        histogram[hour as usize] += tokens;
        active_days.insert(date);
        active_hours.insert(hour);
        total_volume += tokens;
        if matches!(
            date.weekday(),
            time::Weekday::Saturday | time::Weekday::Sunday
        ) {
            weekend_volume += tokens;
        }
    }
    let mature =
        active_days.len() >= MIN_ACTIVE_DAYS && active_hours.len() >= MIN_DISTINCT_ACTIVE_HOURS;
    let windows = if mature {
        derive_phase_windows(&histogram)
    } else {
        PhaseWindows::clock_defaults()
    };
    let weekend_share = if total_volume > 0.0 {
        (weekend_volume / total_volume) as f32
    } else {
        0.0
    };

    // --- phase + boundary instants ---
    let hour = now_local.hour();
    let day_phase = windows.phase_for_hour(hour);
    let (start_hour, end_hour) = phase_bounds_for(&windows, day_phase);
    let phase_started_at_utc = instant_of_local_hour_at_or_before(now, start_hour, mapper);
    let phase_ends_at_utc = instant_of_local_hour_after(now, end_hour, mapper);

    // --- today / yesterday / climate ---
    let today_start = mapper.local_day_start(today);
    let tomorrow_start = mapper.local_day_start(today + Duration::days(1));
    let baseline = state.calibration.daily_effective_tokens.max(1.0);
    let today_tokens = usage_store
        .applied_effective_tokens_between(today_start, now + Duration::seconds(1))
        .unwrap_or(0.0);
    let today_ratio = (today_tokens / baseline) as f32;

    let yesterday = if state.created_at >= today_start {
        None
    } else {
        let y_start = mapper.local_day_start(today - Duration::days(1));
        let tokens = usage_store
            .applied_effective_tokens_between(y_start, today_start)
            .unwrap_or(0.0);
        let shape = usage_store
            .applied_token_shape_between(y_start, today_start)
            .unwrap_or_default();
        Some(DaySummary {
            ratio: (tokens / baseline) as f32,
            dominant_shape: classify_day_shape(shape),
        })
    };

    let climate = {
        let starts = mapper.day_starts_back(now, RHYTHM_WINDOW_DAYS.min(8));
        // last pair is today; the 7 pairs before it are the complete days.
        let mut counts: HashMap<WorkWeather, usize> = HashMap::new();
        let pairs = starts.windows(2).collect::<Vec<_>>();
        for pair in pairs.iter().rev().skip(1).take(7) {
            let shape = usage_store
                .applied_token_shape_between(pair[0], pair[1])
                .unwrap_or_default();
            if let Some(class) = classify_day_shape(shape) {
                *counts.entry(class).or_insert(0) += 1;
            }
        }
        modal_climate(&counts)
    };

    // --- tiredness: accumulated active time, not elapsed span ---
    let tiredness = if mature {
        let fatigue_start = now - Duration::hours(FATIGUE_WINDOW_HOURS);
        let fatigue_buckets = usage_store
            .applied_bucket_sums_between(fatigue_start, now + Duration::seconds(1))
            .unwrap_or_default();
        let active_buckets = fatigue_buckets.iter().filter(|&&(_, t)| t > 0.0).count();
        let window_volume: f64 = fatigue_buckets.iter().map(|&(_, t)| t).sum();
        let active_share = active_buckets as f32 / (FATIGUE_WINDOW_HOURS * 6) as f32;
        let volume_ratio = ((window_volume / baseline) as f32).clamp(0.0, 1.0);
        (active_share * volume_ratio).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // --- sleep predicate (spec: Sleep semantics) ---
    let latest_bucket = usage_store.latest_applied_bucket_at().unwrap_or(None);
    let has_eaten = latest_bucket.is_some();
    let recently_active = latest_bucket
        .map(|b| b >= now - Duration::minutes(SLEEP_IDLE_MINUTES))
        .unwrap_or(false);
    let asleep = day_phase == DayPhase::Night && !recently_active && has_eaten;
    let sleep_onset_utc = if asleep {
        let onset_from_idle = latest_bucket.map(|b| b + Duration::minutes(SLEEP_IDLE_MINUTES));
        Some(match onset_from_idle {
            Some(t) => t.max(phase_started_at_utc),
            None => phase_started_at_utc,
        })
    } else {
        None
    };
    let wake_resume = derive_wake_resume(
        usage_store,
        day_phase,
        asleep,
        latest_bucket,
        phase_started_at_utc,
        now,
    );

    DayContext {
        day_phase,
        phase_started_at_utc,
        phase_ends_at_utc,
        date_seed: date_seed_for(now_local, windows.dawn_start, &state.pet.seed),
        today_ratio,
        yesterday,
        climate,
        is_weekend: matches!(
            now_local.date().weekday(),
            time::Weekday::Saturday | time::Weekday::Sunday
        ),
        weekend_share,
        season: season_for_month(now_local.date().month()),
        mature,
        asleep,
        sleep_onset_utc,
        wake_resume,
        local_day_rollover_utc: tomorrow_start,
        tiredness,
        local_day_started_utc: today_start,
    }
}

/// The poll-path variant for narration: just the asleep bit, same predicate.
pub fn scene_asleep_for_poll(
    usage_store: &UsageStore,
    state: &PetState,
    now: time::OffsetDateTime,
    mapper: LocalDayMapper,
) -> bool {
    build_day_context(usage_store, state, now, mapper).asleep
}

/// Morning-after selection window: all of Dawn plus the first
/// MORNING_AFTER_DAY_MINUTES of Day. Pure function of carried instants.
pub fn in_morning_after_window(day: &DayContext, now: time::OffsetDateTime) -> bool {
    match day.day_phase {
        DayPhase::Dawn => true,
        DayPhase::Day => {
            now < day.phase_started_at_utc + Duration::minutes(MORNING_AFTER_DAY_MINUTES)
        }
        DayPhase::Dusk | DayPhase::Night => false,
    }
}

/// Classify a day's token shape, effective-weighted. The cache-read effective
/// contribution is recovered from stored effective totals (no config access):
/// effective = input + output + cache_creation + weight*cache_read + reasoning.
/// Raw-component all-zero (shape-less legacy rows) and empty days are None —
/// the classifier's Clear must never encode absence.
pub fn classify_day_shape(s: AppliedShapeSums) -> Option<WorkWeather> {
    let raw_components = s.input_tokens
        + s.output_tokens
        + s.cache_creation_tokens
        + s.cache_read_tokens
        + s.reasoning_output_tokens;
    if !raw_components.is_finite() || raw_components <= 0.0 {
        return None;
    }
    let cache_read_effective = (s.effective_tokens
        - s.input_tokens
        - s.output_tokens
        - s.cache_creation_tokens
        - s.reasoning_output_tokens)
        .max(0.0);
    Some(classify_work_weather(Some(TokenShapeDelta {
        input_tokens: s.input_tokens,
        output_tokens: s.output_tokens,
        cache_creation_tokens: s.cache_creation_tokens,
        cache_read_tokens: cache_read_effective,
        reasoning_output_tokens: s.reasoning_output_tokens,
    })))
}

fn modal_climate(counts: &HashMap<WorkWeather, usize>) -> Option<WorkWeather> {
    let max = counts.values().copied().max()?;
    let winners: Vec<WorkWeather> = counts
        .iter()
        .filter(|(_, &c)| c == max)
        .map(|(&w, _)| w)
        .collect();
    match winners.as_slice() {
        [only] => Some(*only),
        _ => Some(WorkWeather::Clear), // tie -> Clear (renders nothing)
    }
}

fn season_for_month(month: time::Month) -> Season {
    use time::Month::*;
    match month {
        December | January | February => Season::Winter,
        March | April | May => Season::Spring,
        June | July | August => Season::Summer,
        September | October | November => Season::Autumn,
    }
}

/// FNV-1a over (dawn-rolled local date, pet seed). Stable across runs and
/// Rust versions (std hashers are not guaranteed stable).
fn date_seed_for(now_local: time::OffsetDateTime, dawn_start: u8, pet_seed: &str) -> u64 {
    let date = if now_local.hour() >= dawn_start {
        now_local.date()
    } else {
        now_local.date() - Duration::days(1)
    };
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in date.to_string().bytes().chain(pet_seed.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

fn phase_bounds_for(windows: &PhaseWindows, phase: DayPhase) -> (u8, u8) {
    match phase {
        DayPhase::Dusk => (windows.dusk_start, windows.night_start),
        DayPhase::Night => (windows.night_start, windows.dawn_start),
        DayPhase::Dawn => (windows.dawn_start, windows.day_start),
        DayPhase::Day => (windows.day_start, windows.dusk_start),
    }
}

/// Most recent UTC instant at which the local clock read `hour`:00.
fn instant_of_local_hour_at_or_before(
    now: time::OffsetDateTime,
    hour: u8,
    mapper: LocalDayMapper,
) -> time::OffsetDateTime {
    let local = now.to_offset(mapper.offset_at(now));
    let date = if local.hour() >= hour {
        local.date()
    } else {
        local.date() - Duration::days(1)
    };
    let offset = mapper.offset_at(mapper.local_day_start(date));
    PrimitiveDateTime::new(date, Time::from_hms(hour, 0, 0).expect("hour < 24"))
        .assume_offset(offset)
}

/// Next UTC instant at which the local clock will read `hour`:00.
fn instant_of_local_hour_after(
    now: time::OffsetDateTime,
    hour: u8,
    mapper: LocalDayMapper,
) -> time::OffsetDateTime {
    let local = now.to_offset(mapper.offset_at(now));
    let date = if local.hour() < hour {
        local.date()
    } else {
        local.date() + Duration::days(1)
    };
    let offset = mapper.offset_at(mapper.local_day_start(date));
    PrimitiveDateTime::new(date, Time::from_hms(hour, 0, 0).expect("hour < 24"))
        .assume_offset(offset)
}

/// Wake-resume: awake at night within the settle window of the apply that
/// woke the pet, with a sleep-qualifying quiet gap before the waking bucket.
fn derive_wake_resume(
    usage_store: &UsageStore,
    day_phase: DayPhase,
    asleep: bool,
    latest_bucket: Option<time::OffsetDateTime>,
    night_started_at: time::OffsetDateTime,
    now: time::OffsetDateTime,
) -> Option<WakeResume> {
    if asleep || day_phase != DayPhase::Night {
        return None;
    }
    let woke_bucket = latest_bucket?;
    let woke_at = usage_store.latest_applied_marked_at().ok().flatten()?;
    if now - woke_at > Duration::seconds(crate::pet::animator::WANDER_SETTLE_SECS + 60) {
        return None; // long past the ease window; no need to carry it
    }
    let prev = usage_store
        .latest_applied_bucket_at_before(woke_bucket)
        .ok()
        .flatten();
    let prev_onset = prev.map(|p| p + Duration::minutes(SLEEP_IDLE_MINUTES));
    let was_asleep = match prev_onset {
        Some(onset) => onset < woke_bucket, // a sleep-qualifying gap preceded the wake
        None => true,                       // first meal of the night after night start
    };
    if !was_asleep {
        return None;
    }
    let from_eval = prev_onset
        .map(|o| o.max(night_started_at))
        .unwrap_or(night_started_at);
    Some(WakeResume {
        from_eval_utc: from_eval,
        woke_at_utc: woke_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::day_axis::LocalDayMapper;
    use crate::storage::usage_store::{NormalizedUsageEvent, UsageStore};
    use time::macros::datetime;

    fn utc_mapper() -> LocalDayMapper {
        LocalDayMapper::Fixed(time::UtcOffset::UTC)
    }

    fn store_with_applied(rows: &[(time::OffsetDateTime, f64)]) -> UsageStore {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        for &(at, tokens) in rows {
            store
                .insert_event(&NormalizedUsageEvent {
                    observed_at: at,
                    bucket_at: at,
                    ..NormalizedUsageEvent::for_test_at(at, tokens)
                })
                .unwrap();
        }
        store
    }

    fn histogram_active(hours: &[u8]) -> [f64; 24] {
        let mut h = [0.0_f64; 24];
        for &hour in hours {
            h[hour as usize] = 1_000.0;
        }
        h
    }

    #[test]
    fn typical_nine_to_six_worker_gets_carved_windows() {
        // Active 9..18 → quiet run 18..9 (15h) → clamped to 12h centered on
        // the quiet midpoint (1.5 ≈ hour 1; window 19..7 with wraparound),
        // dusk = first 2h of the clamped window, dawn = last 2h.
        let w = derive_phase_windows(&histogram_active(&[9, 10, 11, 12, 13, 14, 15, 16, 17]));
        assert_eq!(
            w,
            PhaseWindows {
                dusk_start: 19,
                night_start: 21,
                dawn_start: 5,
                day_start: 7
            }
        );
    }

    #[test]
    fn night_owl_windows_invert() {
        // Active 22..04 (wraps) → quiet run 4..22 (18h) → clamp to 12h
        // centered on quiet midpoint (hour 13; window 7..19).
        let w = derive_phase_windows(&histogram_active(&[22, 23, 0, 1, 2, 3]));
        assert_eq!(
            w,
            PhaseWindows {
                dusk_start: 7,
                night_start: 9,
                dawn_start: 17,
                day_start: 19
            }
        );
    }

    #[test]
    fn short_active_day_user_keeps_a_real_day() {
        // Active only 10..14 → quiet run 14..10 (20h) → clamped to 12h.
        let w = derive_phase_windows(&histogram_active(&[10, 11, 12, 13]));
        // Quiet midpoint of 14..10 is hour 0; clamped window 18..06.
        assert_eq!(
            w,
            PhaseWindows {
                dusk_start: 18,
                night_start: 20,
                dawn_start: 4,
                day_start: 6
            }
        );
        // Day must exist: from day_start (6) to dusk_start (18) = 12 hours.
    }

    #[test]
    fn no_quiet_run_falls_back_to_clock_defaults() {
        let w = derive_phase_windows(&histogram_active(&(0..24).collect::<Vec<_>>()));
        assert_eq!(w, PhaseWindows::clock_defaults());
    }

    #[test]
    fn short_quiet_run_falls_back_to_clock_defaults() {
        // Only a 4-hour quiet gap (< MIN_NIGHT_RUN_HOURS=5).
        let active: Vec<u8> = (0..24).filter(|h| !(2..6).contains(h)).collect();
        let w = derive_phase_windows(&histogram_active(&active));
        assert_eq!(w, PhaseWindows::clock_defaults());
    }

    #[test]
    fn equal_length_quiet_runs_fall_back_to_clock_defaults() {
        // Two 9-hour quiet runs: active at 3,4,5 and 15,16,17 → quiet runs
        // 6..15 and 18..03, both 9h. Ambiguous → defaults.
        let w = derive_phase_windows(&histogram_active(&[3, 4, 5, 15, 16, 17]));
        assert_eq!(w, PhaseWindows::clock_defaults());
    }

    #[test]
    fn split_shift_picks_the_longest_quiet_run() {
        // Active 8..12 and 18..22: quiet runs 12..18 (6h) and 22..8 (10h).
        // Longest = 22..8 (10h, no clamp needed), carve shoulders.
        let w = derive_phase_windows(&histogram_active(&[8, 9, 10, 11, 18, 19, 20, 21]));
        assert_eq!(
            w,
            PhaseWindows {
                dusk_start: 22,
                night_start: 0,
                dawn_start: 6,
                day_start: 8
            }
        );
    }

    #[test]
    fn quiet_share_threshold_ignores_trace_activity() {
        // One hour with 0.5% of total volume still counts as quiet.
        let mut h = histogram_active(&[9, 10, 11, 12, 13, 14, 15, 16, 17]);
        let total: f64 = h.iter().sum();
        h[2] = total * 0.005; // below RHYTHM_QUIET_SHARE = 1%
        let with_trace = derive_phase_windows(&h);
        let without = derive_phase_windows(&histogram_active(&[9, 10, 11, 12, 13, 14, 15, 16, 17]));
        assert_eq!(with_trace, without);
    }

    #[test]
    fn phase_for_hour_maps_circular_ranges() {
        let w = PhaseWindows::clock_defaults(); // dawn 7, day 9, dusk 18, night 22
        assert_eq!(w.phase_for_hour(7), DayPhase::Dawn);
        assert_eq!(w.phase_for_hour(8), DayPhase::Dawn);
        assert_eq!(w.phase_for_hour(9), DayPhase::Day);
        assert_eq!(w.phase_for_hour(17), DayPhase::Day);
        assert_eq!(w.phase_for_hour(18), DayPhase::Dusk);
        assert_eq!(w.phase_for_hour(21), DayPhase::Dusk);
        assert_eq!(w.phase_for_hour(22), DayPhase::Night);
        assert_eq!(w.phase_for_hour(3), DayPhase::Night);
        assert_eq!(w.phase_for_hour(6), DayPhase::Night);
    }

    #[test]
    fn phase_for_hour_handles_wrap_around_window() {
        let w = PhaseWindows {
            dusk_start: 18,
            night_start: 20,
            dawn_start: 4,
            day_start: 6,
        };
        assert_eq!(w.phase_for_hour(19), DayPhase::Dusk);
        assert_eq!(w.phase_for_hour(23), DayPhase::Night);
        assert_eq!(w.phase_for_hour(0), DayPhase::Night);
        assert_eq!(w.phase_for_hour(5), DayPhase::Dawn);
        assert_eq!(w.phase_for_hour(10), DayPhase::Day);
    }

    #[test]
    fn newborn_that_never_ate_stays_awake_at_night() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        let now = datetime!(2026-06-09 23:30 UTC); // clock-default Night
        state.created_at = now - time::Duration::minutes(5);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert_eq!(ctx.day_phase, DayPhase::Night);
        assert!(!ctx.asleep, "a pet that has never eaten must stay awake");
    }

    #[test]
    fn pet_sleeps_after_idle_night_window_and_only_at_night() {
        let now = datetime!(2026-06-09 23:30 UTC);
        let store = store_with_applied(&[(now - time::Duration::hours(3), 5_000.0)]);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(2);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert!(ctx.asleep, "night + 3h ledger quiet => asleep");
        assert_eq!(
            ctx.sleep_onset_utc,
            Some(now - time::Duration::hours(3) + time::Duration::minutes(SLEEP_IDLE_MINUTES))
                .map(|t| t.max(ctx.phase_started_at_utc)),
        );
        // Same quiet ledger at midday: never asleep outside Night.
        let midday = datetime!(2026-06-09 13:00 UTC);
        let ctx2 = build_day_context(&store, &state, midday, utc_mapper());
        assert!(!ctx2.asleep);
    }

    #[test]
    fn recent_tokens_keep_the_pet_awake_including_future_dated_rows() {
        let now = datetime!(2026-06-09 23:30 UTC);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(2);
        let recent = store_with_applied(&[(now - time::Duration::minutes(5), 100.0)]);
        assert!(!build_day_context(&recent, &state, now, utc_mapper()).asleep);
        // Clock-set-backwards: a future bucket counts as recent (fail-awake).
        let future = store_with_applied(&[(now + time::Duration::hours(2), 100.0)]);
        assert!(!build_day_context(&future, &state, now, utc_mapper()).asleep);
    }

    #[test]
    fn today_ratio_divides_by_baseline_and_yesterday_distinguishes_idle_from_no_coverage() {
        let now = datetime!(2026-06-09 12:00 UTC);
        let store = store_with_applied(&[(now - time::Duration::hours(1), 50_000.0)]);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(3);
        // Default test baseline is 100k (CalibrationBaseline::default()).
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert!((ctx.today_ratio - 0.5).abs() < 1e-6);
        // Pet existed yesterday but ate nothing: observed idle day.
        assert_eq!(
            ctx.yesterday,
            Some(DaySummary {
                ratio: 0.0,
                dominant_shape: None
            })
        );
        // Pet created today: no coverage.
        state.created_at = now - time::Duration::hours(1);
        let ctx2 = build_day_context(&store, &state, now, utc_mapper());
        assert_eq!(ctx2.yesterday, None);
    }

    #[test]
    fn day_shape_classification_is_effective_weighted_and_detects_shapeless_rows() {
        // Raw cache dominates (1M cache reads) but its effective contribution
        // is 30k at the 0.03 write-time weight — output should win the class.
        let sums = crate::storage::usage_store::AppliedShapeSums {
            input_tokens: 10_000.0,
            output_tokens: 60_000.0,
            cache_creation_tokens: 0.0,
            cache_read_tokens: 1_000_000.0,
            reasoning_output_tokens: 0.0,
            effective_tokens: 100_000.0, // 10k + 60k + 0.03 * 1M = 100k
        };
        assert_eq!(
            classify_day_shape(sums),
            Some(crate::tui::life::WorkWeather::OutputSparks)
        );
        // Shape-less legacy rows: components zero, effective nonzero -> None.
        let shapeless = crate::storage::usage_store::AppliedShapeSums {
            effective_tokens: 42_000.0,
            ..Default::default()
        };
        assert_eq!(classify_day_shape(shapeless), None);
        // Zero day -> None.
        assert_eq!(
            classify_day_shape(crate::storage::usage_store::AppliedShapeSums::default()),
            None
        );
    }

    #[test]
    fn climate_is_modal_over_prior_days_and_mixed_detail_weeks_ignore_shapeless_days() {
        let now = datetime!(2026-06-09 12:00 UTC);
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        // 3 prior days of output-heavy shape; 2 shape-less days; 2 empty days.
        for back in 1..=3_i64 {
            let at = now - time::Duration::days(back);
            let mut e = NormalizedUsageEvent {
                observed_at: at,
                bucket_at: at,
                ..NormalizedUsageEvent::for_test_at(at, 70_000.0)
            };
            e.input_tokens = 10_000.0;
            e.output_tokens = 60_000.0;
            store.insert_event(&e).unwrap();
        }
        for back in 4..=5_i64 {
            let at = now - time::Duration::days(back);
            store
                .insert_event(&NormalizedUsageEvent {
                    observed_at: at,
                    bucket_at: at,
                    ..NormalizedUsageEvent::for_test_at(at, 9_000.0)
                })
                .unwrap(); // for_test_at leaves components zero: shape-less
        }
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(10);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert_eq!(
            ctx.climate,
            Some(crate::tui::life::WorkWeather::OutputSparks)
        );
    }

    #[test]
    fn maturity_gate_needs_active_days_and_hour_diversity() {
        let now = datetime!(2026-06-09 12:00 UTC);
        let mut rows = Vec::new();
        // 4 active days at 3 distinct hours: below MIN_ACTIVE_DAYS.
        for back in 1..=4_i64 {
            for hour in [9_i64, 13, 17] {
                rows.push((
                    now - time::Duration::days(back) - time::Duration::hours(hour - 12),
                    10_000.0,
                ));
            }
        }
        let store = store_with_applied(&rows);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(10);
        assert!(!build_day_context(&store, &state, now, utc_mapper()).mature);
        // Add a 5th day: exactly MIN_ACTIVE_DAYS with >=3 distinct hours.
        let mut rows5 = rows.clone();
        for hour in [9_i64, 13, 17] {
            rows5.push((
                now - time::Duration::days(5) - time::Duration::hours(hour - 12),
                10_000.0,
            ));
        }
        let store5 = store_with_applied(&rows5);
        assert!(build_day_context(&store5, &state, now, utc_mapper()).mature);
    }

    #[test]
    fn date_seed_rolls_at_dawn_not_midnight() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        let before_dawn = datetime!(2026-06-09 03:00 UTC); // clock defaults: dawn 07
        state.created_at = before_dawn - time::Duration::days(2);
        let after_dawn = datetime!(2026-06-09 08:00 UTC);
        let prev_evening = datetime!(2026-06-08 20:00 UTC);
        let seed_pre = build_day_context(&store, &state, before_dawn, utc_mapper()).date_seed;
        let seed_prev = build_day_context(&store, &state, prev_evening, utc_mapper()).date_seed;
        let seed_post = build_day_context(&store, &state, after_dawn, utc_mapper()).date_seed;
        assert_eq!(
            seed_pre, seed_prev,
            "pre-dawn hours belong to the previous day's character"
        );
        assert_ne!(seed_post, seed_pre, "the day's character rolls at dawn");
    }

    fn applied_event(
        bucket_at: time::OffsetDateTime,
        observed_at: time::OffsetDateTime,
        tokens: f64,
    ) -> NormalizedUsageEvent {
        NormalizedUsageEvent {
            bucket_at,
            observed_at,
            ..NormalizedUsageEvent::for_test_at(bucket_at, tokens)
        }
    }

    #[test]
    fn derive_wake_resume_pins_wake_resume_values() {
        let now = datetime!(2026-06-09 23:30 UTC);
        let night_started_at = datetime!(2026-06-09 22:00 UTC);
        let settle = Duration::seconds(crate::pet::animator::WANDER_SETTLE_SECS + 60);
        let idle = Duration::minutes(SLEEP_IDLE_MINUTES);

        // 1) Wake within settle window after a sleep-qualifying quiet gap.
        let prev_bucket = now - Duration::minutes(50);
        let woke_bucket = now - Duration::minutes(5);
        let woke_observed = now - Duration::seconds(30);
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        store
            .insert_event(&applied_event(prev_bucket, prev_bucket, 1_000.0))
            .unwrap();
        store
            .insert_event(&applied_event(woke_bucket, woke_observed, 1_000.0))
            .unwrap();
        let resume = derive_wake_resume(
            &store,
            DayPhase::Night,
            false,
            Some(woke_bucket),
            night_started_at,
            now,
        );
        assert_eq!(
            resume,
            Some(WakeResume {
                from_eval_utc: prev_bucket + idle,
                woke_at_utc: woke_observed,
            })
        );

        // 2) First meal of the night: no previous bucket -> from_eval equals night start.
        let mut store2 = UsageStore::open(":memory:".as_ref()).unwrap();
        store2
            .insert_event(&applied_event(woke_bucket, woke_observed, 1_000.0))
            .unwrap();
        let resume2 = derive_wake_resume(
            &store2,
            DayPhase::Night,
            false,
            Some(woke_bucket),
            night_started_at,
            now,
        );
        assert_eq!(
            resume2,
            Some(WakeResume {
                from_eval_utc: night_started_at,
                woke_at_utc: woke_observed,
            })
        );

        // 3) Previous bucket too recent: no sleep-qualifying gap -> None.
        let recent_prev = now - Duration::minutes(10);
        let mut store3 = UsageStore::open(":memory:".as_ref()).unwrap();
        store3
            .insert_event(&applied_event(recent_prev, recent_prev, 1_000.0))
            .unwrap();
        store3
            .insert_event(&applied_event(woke_bucket, woke_observed, 1_000.0))
            .unwrap();
        let resume3 = derive_wake_resume(
            &store3,
            DayPhase::Night,
            false,
            Some(woke_bucket),
            night_started_at,
            now,
        );
        assert_eq!(resume3, None);

        // 4) After the settle window expires -> None.
        let stale_observed = now - (settle + Duration::seconds(1));
        let mut store4 = UsageStore::open(":memory:".as_ref()).unwrap();
        store4
            .insert_event(&applied_event(prev_bucket, prev_bucket, 1_000.0))
            .unwrap();
        store4
            .insert_event(&applied_event(woke_bucket, stale_observed, 1_000.0))
            .unwrap();
        let resume4 = derive_wake_resume(
            &store4,
            DayPhase::Night,
            false,
            Some(woke_bucket),
            night_started_at,
            now,
        );
        assert_eq!(resume4, None);
    }

    /// Rows that satisfy the maturity gate (5 distinct active days, 3 distinct
    /// hours) while staying outside the trailing FATIGUE_WINDOW_HOURS at `now`
    /// AND outside today/yesterday: 2..=6 local days back.
    fn maturity_rows(now: time::OffsetDateTime) -> Vec<(time::OffsetDateTime, f64)> {
        let mut rows = Vec::new();
        for back in 2..=6_i64 {
            for hour in [9_i64, 13, 17] {
                rows.push((
                    now - time::Duration::days(back) - time::Duration::hours(hour - 12),
                    10_000.0,
                ));
            }
        }
        rows
    }

    #[test]
    fn tiredness_counts_active_buckets_not_elapsed_span() {
        let now = datetime!(2026-06-09 16:00 UTC);
        let mut rows = maturity_rows(now);
        let morning_start = datetime!(2026-06-09 06:00 UTC);
        for i in 0..24_i64 {
            rows.push((morning_start + time::Duration::minutes(i * 10), 20_000.0));
        }
        let store = store_with_applied(&rows);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(10);

        let at_four_pm = build_day_context(&store, &state, now, utc_mapper());
        assert!(at_four_pm.mature, "fixture must pass the maturity gate");
        assert!(
            (at_four_pm.tiredness - 0.25).abs() < 1e-6,
            "got {}",
            at_four_pm.tiredness
        );
        assert!(
            at_four_pm.tiredness < 0.5,
            "a rested afternoon must never read near-max"
        );

        let at_ten_am = build_day_context(
            &store,
            &state,
            datetime!(2026-06-09 10:00 UTC),
            utc_mapper(),
        );
        assert!((at_ten_am.tiredness - at_four_pm.tiredness).abs() < 1e-6);
    }

    #[test]
    fn light_days_stay_below_the_tired_motion_threshold() {
        let now = datetime!(2026-06-09 12:00 UTC);
        let mut rows = maturity_rows(now);
        for i in 0..3_i64 {
            rows.push((
                now - time::Duration::hours(2) + time::Duration::minutes(i * 10),
                2_000.0,
            ));
        }
        let store = store_with_applied(&rows);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(10);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert!(ctx.mature);
        assert!(ctx.tiredness > 0.0, "real activity must register");
        assert!(
            ctx.tiredness < 0.05,
            "light days must not read tired, got {}",
            ctx.tiredness
        );
    }

    #[test]
    fn tiredness_is_zero_while_the_maturity_gate_is_closed() {
        let now = datetime!(2026-06-09 16:00 UTC);
        let mut rows = Vec::new();
        for i in 0..24_i64 {
            rows.push((
                now - time::Duration::hours(5) + time::Duration::minutes(i * 10),
                20_000.0,
            ));
        }
        let store = store_with_applied(&rows);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(10);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert!(!ctx.mature);
        assert_eq!(ctx.tiredness, 0.0, "immature pets never read tired");
    }

    #[test]
    fn local_day_started_utc_is_the_current_local_midnight() {
        let now = datetime!(2026-06-09 16:00 UTC);
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(2);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert_eq!(ctx.local_day_started_utc, datetime!(2026-06-09 00:00 UTC));
        assert_eq!(ctx.local_day_rollover_utc, datetime!(2026-06-10 00:00 UTC));
        let minus8 = LocalDayMapper::Fixed(time::UtcOffset::from_hms(-8, 0, 0).unwrap());
        let ctx_pst = build_day_context(&store, &state, now, minus8);
        assert_eq!(
            ctx_pst.local_day_started_utc,
            datetime!(2026-06-09 08:00 UTC),
            "local midnight at UTC-8 is 08:00 UTC"
        );
    }

    #[test]
    fn morning_after_window_covers_dawn_plus_first_day_hour_and_is_restart_idempotent() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = datetime!(2026-06-07 00:00 UTC);
        let cases = [
            (datetime!(2026-06-09 07:30 UTC), true),  // mid-Dawn
            (datetime!(2026-06-09 08:59 UTC), true),  // last Dawn minute
            (datetime!(2026-06-09 09:30 UTC), true),  // 30 min into Day
            (datetime!(2026-06-09 10:30 UTC), false), // 90 min into Day
            (datetime!(2026-06-09 20:00 UTC), false), // Dusk
            (datetime!(2026-06-09 02:00 UTC), false), // Night
        ];
        for (now, expected) in cases {
            let ctx = build_day_context(&store, &state, now, utc_mapper());
            assert_eq!(in_morning_after_window(&ctx, now), expected, "at {now}");
            let rebuilt = build_day_context(&store, &state, now, utc_mapper());
            assert_eq!(
                in_morning_after_window(&rebuilt, now),
                in_morning_after_window(&ctx, now)
            );
        }
    }

    #[test]
    fn day_context_carries_the_local_day_started_instant() {
        let now = datetime!(2026-06-09 15:00 UTC);
        let store = store_with_applied(&[(now - time::Duration::hours(1), 5_000.0)]);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(2);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert_eq!(ctx.local_day_started_utc, datetime!(2026-06-09 00:00 UTC));
        assert_eq!(ctx.local_day_rollover_utc, datetime!(2026-06-10 00:00 UTC));
        assert_eq!(
            DayContext::default().local_day_started_utc,
            time::OffsetDateTime::UNIX_EPOCH
        );
    }
}
