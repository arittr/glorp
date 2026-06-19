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
