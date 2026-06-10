//! Day phase and rhythm derivation for the time-of-day presentation layer.
//!
//! `DayContext` itself is built in Task 6; this module provides the
//! phase-window constants and algorithm it consumes.
//!
//! See docs/superpowers/specs/2026-06-09-glorp-lives-in-time-design.md.
//! Everything here is a pure function of (clock, mapper, ledger aggregates);
//! nothing is persisted.

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
