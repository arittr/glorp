use crate::commands::watch::{
    build_watch_view_model, build_watch_view_model_semantic, poll_usage_and_apply,
};
use crate::paths::AppPaths;
use crate::storage::state::{PetState, StateStore};
use crate::tui::life::{AppliedUsageSignal, LifeSignalState};
use crate::tui::view_model::WatchViewModel;
use std::sync::mpsc;
use std::thread;
use std::time::Duration as StdDuration;
use time::OffsetDateTime;

/// Owns the live presentation state shared by menubar and companion facades.
#[derive(Debug, Default)]
pub struct WatchPresentationState {
    life_signal_state: LifeSignalState,
}

/// Snapshot of fresh provider data computed on the worker thread.
pub struct LiveWatchUpdate {
    pub pet_state: PetState,
    pub vm: WatchViewModel,
    pub applied_signal: AppliedUsageSignal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveWatchRenderMode {
    Rendered,
    Semantic,
}

/// Applies a usage signal to the shared presentation state and stamps the
/// resulting profile onto the view model. Sets `last_feed_pulse_at` only when
/// the signal can burst.
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

/// Spawns a background thread that polls usage and emits `LiveWatchUpdate`s.
/// Silently skips poll/build failures so the facade keeps showing the last good
/// state; this matches the existing menubar behavior and the V1 spec.
pub fn spawn_live_watch_worker(
    paths: AppPaths,
    interval: StdDuration,
    name: &str,
    render_mode: LiveWatchRenderMode,
) -> mpsc::Receiver<LiveWatchUpdate> {
    let (tx, rx) = mpsc::channel::<LiveWatchUpdate>();
    let thread_name = name.to_string();
    thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            let state_store = StateStore::new(paths.state_file.clone());
            loop {
                // V1 waits one interval before the first poll; the initial scene is loaded
                // by the app before the run loop starts.
                thread::sleep(interval);
                let outcome =
                    match poll_usage_and_apply(&state_store, &paths.usage_db, &paths.config_file) {
                        Ok(Some(outcome)) => outcome,
                        // Silently skip poll/build failures: the facade keeps showing the last good
                        // state until the next successful poll. This matches the existing menubar
                        // behavior and the V1 spec.
                        Ok(None) | Err(_) => continue,
                    };
                let vm = match match render_mode {
                    LiveWatchRenderMode::Rendered => {
                        build_watch_view_model(&outcome.state, &paths.usage_db)
                    }
                    LiveWatchRenderMode::Semantic => {
                        build_watch_view_model_semantic(&outcome.state, &paths.usage_db)
                    }
                } {
                    Ok(vm) => vm,
                    // Silently skip poll/build failures: the facade keeps showing the last good
                    // state until the next successful poll. This matches the existing menubar
                    // behavior and the V1 spec.
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
