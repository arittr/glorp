use crate::error::{GlorpError, Result};
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset, Weekday};

pub const TOKENMAXXING_TIMEZONE: &str = "America/Los_Angeles";

pub fn parse_agentsview_period_date(period: &str) -> Result<(Date, OffsetDateTime)> {
    let mut parts = period.split('-');
    let year = parts
        .next()
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| GlorpError::Message(format!("invalid agentsview period date {period}")))?;
    let month = parts
        .next()
        .and_then(|value| value.parse::<u8>().ok())
        .and_then(|value| Month::try_from(value).ok())
        .ok_or_else(|| GlorpError::Message(format!("invalid agentsview period date {period}")))?;
    let day = parts
        .next()
        .and_then(|value| value.parse::<u8>().ok())
        .ok_or_else(|| GlorpError::Message(format!("invalid agentsview period date {period}")))?;
    if parts.next().is_some() {
        return Err(GlorpError::Message(format!(
            "invalid agentsview period date {period}"
        )));
    }
    let date = Date::from_calendar_date(year, month, day).map_err(|err| {
        GlorpError::Message(format!("invalid agentsview period date {period}: {err}"))
    })?;
    Ok((date, tokenmaxxing_day_start(date)))
}

pub fn tokenmaxxing_today_window(now: OffsetDateTime) -> (OffsetDateTime, OffsetDateTime) {
    let date = tokenmaxxing_date(now);
    (
        tokenmaxxing_day_start(date),
        tokenmaxxing_day_start(date + time::Duration::days(1)),
    )
}

pub fn tokenmaxxing_day_start(date: Date) -> OffsetDateTime {
    PrimitiveDateTime::new(date, Time::MIDNIGHT)
        .assume_offset(los_angeles_offset_for_local_midnight(date))
        .to_offset(UtcOffset::UTC)
}

pub fn tokenmaxxing_provider_day(now: OffsetDateTime) -> Date {
    tokenmaxxing_date(now)
}

pub fn tokenmaxxing_days_back(now: OffsetDateTime, count: usize) -> Vec<Date> {
    let today = tokenmaxxing_provider_day(now);
    let start = count.saturating_sub(1) as i64;
    (0..count)
        .map(|index| today - time::Duration::days(start - index as i64))
        .collect()
}

fn tokenmaxxing_date(now: OffsetDateTime) -> Date {
    now.to_offset(los_angeles_offset_for_instant(now)).date()
}

fn los_angeles_offset_for_local_midnight(date: Date) -> UtcOffset {
    let start = nth_weekday_of_month(date.year(), Month::March, Weekday::Sunday, 2);
    let end = nth_weekday_of_month(date.year(), Month::November, Weekday::Sunday, 1);
    if date > start && date <= end {
        UtcOffset::from_hms(-7, 0, 0).unwrap()
    } else {
        UtcOffset::from_hms(-8, 0, 0).unwrap()
    }
}

fn los_angeles_offset_for_instant(instant: OffsetDateTime) -> UtcOffset {
    let pacific = instant
        .to_offset(UtcOffset::from_hms(-8, 0, 0).unwrap())
        .date();
    let start = nth_weekday_of_month(pacific.year(), Month::March, Weekday::Sunday, 2);
    let end = nth_weekday_of_month(pacific.year(), Month::November, Weekday::Sunday, 1);
    let dst_start_utc = PrimitiveDateTime::new(start, Time::from_hms(2, 0, 0).unwrap())
        .assume_offset(UtcOffset::from_hms(-8, 0, 0).unwrap())
        .to_offset(UtcOffset::UTC);
    let dst_end_utc = PrimitiveDateTime::new(end, Time::from_hms(2, 0, 0).unwrap())
        .assume_offset(UtcOffset::from_hms(-7, 0, 0).unwrap())
        .to_offset(UtcOffset::UTC);
    if instant >= dst_start_utc && instant < dst_end_utc {
        UtcOffset::from_hms(-7, 0, 0).unwrap()
    } else {
        UtcOffset::from_hms(-8, 0, 0).unwrap()
    }
}

fn nth_weekday_of_month(year: i32, month: Month, weekday: Weekday, nth: u8) -> Date {
    let mut date = Date::from_calendar_date(year, month, 1).unwrap();
    let mut seen = 0_u8;
    loop {
        if date.weekday() == weekday {
            seen += 1;
            if seen == nth {
                return date;
            }
        }
        date += time::Duration::days(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_day_uses_los_angeles_date() {
        let before_la_midnight = time::macros::datetime!(2026 - 07 - 06 06:59:00 UTC);
        let after_la_midnight = time::macros::datetime!(2026 - 07 - 06 07:00:00 UTC);

        assert_eq!(
            tokenmaxxing_provider_day(before_la_midnight),
            time::macros::date!(2026 - 07 - 05)
        );
        assert_eq!(
            tokenmaxxing_provider_day(after_la_midnight),
            time::macros::date!(2026 - 07 - 06)
        );
    }

    #[test]
    fn days_back_returns_oldest_to_newest_provider_days() {
        let now = time::macros::datetime!(2026 - 07 - 06 20:00 UTC);
        assert_eq!(
            tokenmaxxing_days_back(now, 3),
            vec![
                time::macros::date!(2026 - 07 - 04),
                time::macros::date!(2026 - 07 - 05),
                time::macros::date!(2026 - 07 - 06),
            ]
        );
    }
}
