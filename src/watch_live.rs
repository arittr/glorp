use crate::commands::watch::{build_watch_view_model, poll_usage_and_apply};
use crate::paths::AppPaths;
use crate::storage::state::{PetState, StateStore};
use crate::tui::life::{AppliedUsageSignal, LifeSignalState};
use crate::tui::view_model::WatchViewModel;
use std::sync::mpsc;
use std::thread;
use std::time::Duration as StdDuration;
use time::OffsetDateTime;

#[derive(Debug, Default)]
pub struct WatchPresentationState {
    life_signal_state: LifeSignalState,
}

pub struct LiveWatchUpdate {
    pub pet_state: PetState,
    pub vm: WatchViewModel,
    pub applied_signal: AppliedUsageSignal,
}

pub fn stamp_live_presentation(
    state: &mut WatchPresentationState,
    vm: &mut WatchViewModel,
    applied_signal: AppliedUsageSignal,
    now: OffsetDateTime,
) {
    let profile = state
        .life_signal_state
        .observe(applied_signal, &vm.activity_identity, now);
    vm.life_profile = profile;
    vm.life_profile.calm_mode = vm.day_context.asleep;
    vm.last_feed_pulse_at = applied_signal.can_burst().then_some(now);
}

pub fn spawn_live_watch_worker(
    paths: AppPaths,
    interval: StdDuration,
    name: &str,
) -> mpsc::Receiver<LiveWatchUpdate> {
    let (tx, rx) = mpsc::channel::<LiveWatchUpdate>();
    let thread_name = name.to_string();
    thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            let state_store = StateStore::new(paths.state_file.clone());
            loop {
                thread::sleep(interval);
                let outcome =
                    match poll_usage_and_apply(&state_store, &paths.usage_db, &paths.config_file) {
                        Ok(Some(outcome)) => outcome,
                        Ok(None) | Err(_) => continue,
                    };
                let vm = match build_watch_view_model(&outcome.state, &paths.usage_db) {
                    Ok(vm) => vm,
                    Err(_) => continue,
                };
                if tx
                    .send(LiveWatchUpdate {
                        pet_state: outcome.state,
                        vm,
                        applied_signal: outcome.applied_signal,
                    })
                    .is_err()
                {
                    return;
                }
            }
        })
        .expect("spawn glorp live watch worker");
    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::life::AppliedUsageSignal;
    use crate::tui::view_model::WatchViewModel;
    use time::macros::datetime;

    #[test]
    fn install_live_signal_sets_feed_pulse_only_for_bursting_usage() {
        let mut state = WatchPresentationState::default();
        let mut vm = WatchViewModel::fixture();
        let now = datetime!(2026-06-13 18:00 UTC);

        // Seed last_observed_at so the life-state machine treats the next
        // signal as Live rather than ColdStart.
        stamp_live_presentation(
            &mut state,
            &mut vm,
            AppliedUsageSignal::diagnostics_only(now, time::Duration::minutes(1)),
            now,
        );

        let signal = AppliedUsageSignal {
            applied_effective_tokens: 42_000.0,
            raw_effective_tokens: Some(42_000.0),
            source_mix: None,
            token_shape: None,
            observed_at: now,
            elapsed_since_successful_poll: time::Duration::minutes(1),
            freshness: crate::tui::life::UsageSignalFreshness::Live,
        };

        stamp_live_presentation(&mut state, &mut vm, signal, now);

        assert!(vm.life_profile.activity_level > 0.0);
        assert_eq!(vm.last_feed_pulse_at, Some(now));
    }

    #[test]
    fn diagnostics_only_signal_does_not_create_feed_pulse() {
        let mut state = WatchPresentationState::default();
        let mut vm = WatchViewModel::fixture();
        let now = datetime!(2026-06-13 18:00 UTC);

        stamp_live_presentation(
            &mut state,
            &mut vm,
            AppliedUsageSignal::diagnostics_only(now, time::Duration::minutes(1)),
            now,
        );

        assert_eq!(vm.last_feed_pulse_at, None);
    }
}
