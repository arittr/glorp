//! Local-day axis mapping (the timezone seam).
//!
//! All "today / yesterday / trailing-N-day" math goes through one injectable
//! mapper so unit tests and Preview Lab can pin offsets while production
//! resolves the OS timezone. `System` resolves the UTC offset per calendar-day
//! boundary (one `localtime_r` per day in a window, never per row), so DST
//! days group correctly. Resolution failure falls back to UTC — a named
//! decision in the spec, not an accident.

use time::{Date, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Function pointer equality is only used to distinguish identical test fixtures;
// cross-pointer equality is not meaningful for this type.
#[allow(unpredictable_function_pointer_comparisons)]
pub enum LocalDayMapper {
    /// One constant offset. Tests, Preview Lab, and dev fixtures.
    Fixed(UtcOffset),
    /// Resolve via the OS per requested instant (UTC fallback on failure).
    System,
    /// Offset as a pure function of the instant. DST tests.
    Scripted(fn(OffsetDateTime) -> UtcOffset),
}

impl LocalDayMapper {
    pub fn offset_at(self, instant: OffsetDateTime) -> UtcOffset {
        match self {
            Self::Fixed(offset) => offset,
            Self::System => UtcOffset::local_offset_at(instant).unwrap_or(UtcOffset::UTC),
            Self::Scripted(f) => f(instant),
        }
    }

    /// The local calendar date containing `instant`.
    pub fn local_date(self, instant: OffsetDateTime) -> Date {
        instant.to_offset(self.offset_at(instant)).date()
    }

    /// Local hour-of-day (0-23) of `instant`.
    pub fn local_hour(self, instant: OffsetDateTime) -> u8 {
        instant.to_offset(self.offset_at(instant)).hour()
    }

    /// UTC instant of local midnight starting `date`. The offset is resolved
    /// at the boundary itself and iterated until it stabilises, so DST
    /// transitions that fall between local midnight and `date 00:00 UTC`
    /// (common for positive offsets) are handled correctly.
    pub fn local_day_start(self, date: Date) -> OffsetDateTime {
        let mut candidate = PrimitiveDateTime::new(date, Time::MIDNIGHT).assume_utc();
        for _ in 0..4 {
            let offset = self.offset_at(candidate);
            let next = PrimitiveDateTime::new(date, Time::MIDNIGHT).assume_offset(offset);
            if next == candidate {
                return next;
            }
            candidate = next;
        }
        candidate
    }

    /// Ascending UTC boundaries for the `days_back` local days ending with the
    /// day containing `now`. Returns `days_back + 1` instants: index `i` is
    /// the start of day `i`, and the final element is the exclusive end of the
    /// last day (start of tomorrow). Day windows are half-open
    /// `[starts[i], starts[i+1])`.
    pub fn day_starts_back(self, now: OffsetDateTime, days_back: usize) -> Vec<OffsetDateTime> {
        debug_assert!(days_back >= 1, "day_starts_back requires days_back >= 1");
        let today = self.local_date(now);
        let mut starts = Vec::with_capacity(days_back + 1);
        for i in (0..days_back).rev() {
            let date = today - time::Duration::days(i as i64);
            starts.push(self.local_day_start(date));
        }
        starts.push(self.local_day_start(today + time::Duration::days(1)));
        starts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn fixed_mapper_maps_instants_to_local_dates() {
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::from_hms(-8, 0, 0).unwrap());
        // 2026-06-09 03:00 UTC is 2026-06-08 19:00 local at UTC-8.
        let instant = datetime!(2026-06-09 03:00 UTC);
        assert_eq!(
            mapper.local_date(instant),
            time::Date::from_calendar_date(2026, time::Month::June, 8).unwrap()
        );
    }

    #[test]
    fn local_day_start_is_local_midnight_in_utc() {
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::from_hms(-8, 0, 0).unwrap());
        let date = time::Date::from_calendar_date(2026, time::Month::June, 8).unwrap();
        // Local midnight at UTC-8 == 08:00 UTC the same calendar day.
        assert_eq!(
            mapper.local_day_start(date),
            datetime!(2026-06-08 08:00 UTC)
        );
    }

    #[test]
    fn day_starts_back_returns_ascending_boundaries_inclusive_of_today() {
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::UTC);
        let now = datetime!(2026-06-09 15:00 UTC);
        let starts = mapper.day_starts_back(now, 3);
        assert_eq!(
            starts,
            vec![
                datetime!(2026-06-07 00:00 UTC),
                datetime!(2026-06-08 00:00 UTC),
                datetime!(2026-06-09 00:00 UTC),
                datetime!(2026-06-10 00:00 UTC), // exclusive end boundary
            ]
        );
    }

    #[test]
    fn scripted_mapper_groups_dst_days_correctly() {
        // Simulated US spring-forward: UTC-8 before 2026-03-08 10:00 UTC, UTC-7 after.
        // The critical case is local midnight on March 10: under UTC-7 it is 07:00 UTC,
        // but under a stale UTC-8 reading it would be 08:00 UTC. A row at 07:30 UTC
        // therefore belongs to March 10 post-DST, but would be grouped into March 9
        // if we used the wrong offset.
        fn offset(at: time::OffsetDateTime) -> time::UtcOffset {
            if at < datetime!(2026-03-08 10:00 UTC) {
                time::UtcOffset::from_hms(-8, 0, 0).unwrap()
            } else {
                time::UtcOffset::from_hms(-7, 0, 0).unwrap()
            }
        }
        let mapper = LocalDayMapper::Scripted(offset);
        let row = datetime!(2026-03-10 07:30 UTC);
        assert_eq!(
            mapper.local_date(row),
            time::Date::from_calendar_date(2026, time::Month::March, 10).unwrap()
        );
        // Boundary instants on either side of the transition use their own
        // day's offset:
        let pre = time::Date::from_calendar_date(2026, time::Month::March, 7).unwrap();
        let post = time::Date::from_calendar_date(2026, time::Month::March, 10).unwrap();
        assert_eq!(mapper.local_day_start(pre), datetime!(2026-03-07 08:00 UTC));
        assert_eq!(
            mapper.local_day_start(post),
            datetime!(2026-03-10 07:00 UTC)
        );
    }

    #[test]
    fn local_hour_uses_the_instants_own_offset() {
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::from_hms(5, 30, 0).unwrap());
        let instant = datetime!(2026-06-09 03:00 UTC); // 08:30 local
        assert_eq!(mapper.local_hour(instant), 8);
    }

    #[test]
    fn scripted_mapper_handles_nz_spring_forward_at_positive_offset() {
        // NZ spring-forward: +12 before 2026-09-26 14:00 UTC, +13 after.
        // Local midnight on 2026-09-27 begins before the transition, so the
        // boundary must be computed with the pre-transition +12 offset:
        // 2026-09-27 00:00 +12 == 2026-09-26 12:00 UTC.
        fn offset(at: time::OffsetDateTime) -> time::UtcOffset {
            if at < datetime!(2026-09-26 14:00 UTC) {
                time::UtcOffset::from_hms(12, 0, 0).unwrap()
            } else {
                time::UtcOffset::from_hms(13, 0, 0).unwrap()
            }
        }
        let mapper = LocalDayMapper::Scripted(offset);
        let date = time::Date::from_calendar_date(2026, time::Month::September, 27).unwrap();
        assert_eq!(
            mapper.local_day_start(date),
            datetime!(2026-09-26 12:00 UTC)
        );
    }
}
