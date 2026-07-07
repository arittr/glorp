pub fn now_utc() -> time::OffsetDateTime {
    time::OffsetDateTime::now_utc()
}

pub(crate) fn usage_now_utc() -> time::OffsetDateTime {
    #[cfg(any(test, debug_assertions))]
    if let Some(now) = usage_now_from_environment_for_test() {
        return now;
    }

    time::OffsetDateTime::now_utc()
}

#[cfg(any(test, debug_assertions))]
fn usage_now_from_environment_for_test() -> Option<time::OffsetDateTime> {
    let raw = std::env::var("GLORP_USAGE_NOW_FOR_TEST").ok()?;
    time::OffsetDateTime::parse(&raw, &time::format_description::well_known::Rfc3339).ok()
}
