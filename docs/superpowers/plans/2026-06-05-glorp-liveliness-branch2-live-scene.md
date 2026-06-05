# Glorp Liveliness Branch 2 Live Scene Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make high-stage Glorp pets visibly respond to real work sessions through a privacy-preserving live-scene profile, with the data contract proven before renderer behavior depends on it.

**Architecture:** Add a numeric `PetLifeProfile` presentation contract on `WatchViewModel`, backed by an explicit applied usage signal and an in-memory `LifeSignalState` owned by the TUI and menubar loops. Provider/runtime code carries deduped applied token/source/bucket details; rendering code consumes only the profile in panel/component layers and keeps `render_pet` semantic and content-agnostic.

**Tech Stack:** Rust, ratatui, SQLite via `rusqlite`, `time`, `serde`, hidden `dev-preview`, `insta`, cargo test/fmt/clippy.

**Spec:** `docs/superpowers/specs/2026-06-05-glorp-liveliness-branch2-design.md`

**Linear:** PRI-2072

**Branch/worktree:** Do not create a branch unless Drew explicitly asks. If execution happens in an isolated worktree, create it at execution time with `superpowers:using-git-worktrees`.

**Commit convention:** Conventional commits. Commit after each task that leaves tests green.

---

## Background the Executor Needs

- `RuntimeUpdate.recent_effective_tokens` is not the live signal. It is computed after unapplied ledger rows are read and catchup-smearing has already happened in `src/game/runtime.rs`.
- `UsageDelta` currently lacks token bucket fields even though `RawTokenTotals` already exists in `src/usage/normalize.rs`; runtime currently writes `0.0` token-shape buckets in `event_for_delta`.
- `WatchUsagePoller` currently returns only `WatchViewModel`; `WatchApp` swaps that VM directly. The live signal needs a poll envelope so the app can update `LifeSignalState` before installing the new VM.
- Menubar has its own poll worker. It should own its own `LifeSignalState` and should stay poll-bound, not per-tick animated, unless a rendered-block diff signature is added.
- `render_pet` must not receive live usage details. Live visual behavior belongs in `PetPanel`, `habitat_props_for`, speech/feed selection, and constrained menubar style mapping.
- Preview Lab must prove liveliness with deterministic fixtures and `.cells.json` assertions, not only whole-frame snapshots.

## File Structure

- **Create** `src/tui/life.rs` — `PetLifeProfile`, `AppliedUsageSignal`, `LifeSignalState`, classifiers, profile defaults, and unit tests.
- **Modify** `src/tui/mod.rs` — export the `life` module.
- **Modify** `src/tui/view_model.rs` — add `life_profile: PetLifeProfile` to `WatchViewModel` and fixture defaults.
- **Modify** `src/usage/provider.rs` — add optional token bucket totals to `UsageDelta`.
- **Modify** `src/usage/ccusage.rs` — carry `RawTokenTotals` delta buckets into `UsageDelta`.
- **Modify** `src/game/runtime.rs` — apportion token buckets across catchup buckets, derive `AppliedUsageSignal`, and return it with `RuntimeUpdate`.
- **Modify** `src/commands/watch.rs` — return poll envelopes, build/stamp profiles, keep `rerender_pet_for_view_model` semantic-only.
- **Modify** `src/tui/app.rs` — change poller/result types, own `LifeSignalState`, and update profile after each poll.
- **Modify** `src/menubar/app.rs` — mirror the poll-envelope/profile flow for menubar.
- **Modify** `src/menubar/render.rs` — use profile only for poll-bound color/accent choices while preserving BMP/char-length assumptions.
- **Modify** `src/tui/panels/pet.rs` — consume `PetLifeProfile` for brightness, capped ambient glyphs, weather, and burst visual intensity.
- **Modify** `src/tui/component/habitat_props.rs` — consume prop reactions for earned visible props only.
- **Modify** `src/pet/speech.rs` — select activity-aware speech from `PetLifeProfile`.
- **Modify** `src/pet/activity.rs` — select sparse activity lines from `PetLifeProfile` and keep token-added rows intact.
- **Modify** `src/dev_preview/watch.rs` — add deterministic liveliness fixtures.
- **Modify** `src/dev_preview/scenarios.rs` — include profile inputs in manifest entries.
- **Modify** `tests/dev_preview.rs` — add targeted `.cells.json`/layout assertions for liveliness fixtures.
- **Create** `docs/superpowers/measurements/2026-06-05-glorp-life-normalization.md` — record the chosen normalization constants and observed ranges.

---

### Task 1: Add the Live Profile Types With No Behavior Change

**Files:**
- Create: `src/tui/life.rs`
- Modify: `src/tui/mod.rs`
- Modify: `src/tui/view_model.rs`

- [ ] **Step 1: Add the profile module and default-profile test**

In `src/tui/mod.rs`, add:

```rust
pub mod life;
```

Create `src/tui/life.rs` with the type skeleton and this test module.

```rust
use time::{Duration, OffsetDateTime};

use crate::storage::state::HabitatPropId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppliedUsageSignal {
    pub applied_effective_tokens: f64,
    pub raw_effective_tokens: Option<f64>,
    pub source_mix: Option<AppliedSourceMix>,
    pub token_shape: Option<TokenShapeDelta>,
    pub observed_at: OffsetDateTime,
    pub elapsed_since_successful_poll: Duration,
    pub freshness: UsageSignalFreshness,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppliedSourceMix {
    pub claude_effective_tokens: f64,
    pub codex_effective_tokens: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TokenShapeDelta {
    pub input_tokens: f64,
    pub output_tokens: f64,
    pub cache_creation_tokens: f64,
    pub cache_read_tokens: f64,
    pub reasoning_output_tokens: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageSignalFreshness {
    Live,
    ColdStart,
    Backfill,
    DiagnosticsOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceAccent {
    Claude,
    Codex,
    Balanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkWeather {
    Clear,
    CacheMist,
    OutputSparks,
    ReasoningPulse,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropReactionKind {
    Glow,
    Bloom,
    Pulse,
    Orbit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropReaction {
    pub prop_id: HabitatPropId,
    pub intensity: f32,
    pub kind: PropReactionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdleLifeState {
    pub idle_minutes: u32,
    pub is_recently_active: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PetLifeProfile {
    pub activity_level: f32,
    pub burst_level: f32,
    pub source_accent: Option<SourceAccent>,
    pub work_weather: WorkWeather,
    pub prop_reactions: Vec<PropReaction>,
    pub idle: IdleLifeState,
    pub calm_mode: bool,
}

impl PetLifeProfile {
    pub fn idle() -> Self {
        Self {
            activity_level: 0.0,
            burst_level: 0.0,
            source_accent: None,
            work_weather: WorkWeather::Clear,
            prop_reactions: Vec::new(),
            idle: IdleLifeState {
                idle_minutes: 0,
                is_recently_active: false,
            },
            calm_mode: false,
        }
    }
}

impl Default for PetLifeProfile {
    fn default() -> Self {
        Self::idle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_is_quiet_and_clear() {
        let profile = PetLifeProfile::default();

        assert_eq!(profile.activity_level, 0.0);
        assert_eq!(profile.burst_level, 0.0);
        assert_eq!(profile.source_accent, None);
        assert_eq!(profile.work_weather, WorkWeather::Clear);
        assert!(profile.prop_reactions.is_empty());
        assert!(!profile.idle.is_recently_active);
        assert!(!profile.calm_mode);
    }
}
```

- [ ] **Step 2: Run the profile contract test**

Run: `cargo test --lib tui::life::tests::default_profile_is_quiet_and_clear`

Expected: PASS. This establishes the new profile contract before wiring it into
the view model.

- [ ] **Step 3: Write the failing view-model fixture test**

In `src/tui/view_model.rs`, extend `watch_view_model_fixture_has_progress_and_bio`:

```rust
        assert_eq!(
            vm.life_profile,
            crate::tui::life::PetLifeProfile::default(),
            "fixture should start with the quiet live profile"
        );
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test --lib watch_view_model_fixture_has_progress_and_bio`

Expected: FAIL because `WatchViewModel` does not yet have `life_profile`.

- [ ] **Step 5: Add the profile to the watch view model**

In `src/tui/view_model.rs`, import the profile and add it to `WatchViewModel`:

```rust
use crate::tui::life::PetLifeProfile;
```

Add this field near the pet render/habitat fields:

```rust
    pub life_profile: PetLifeProfile,
```

Update every `WatchViewModel` fixture/constructor in the same file to include:

```rust
        life_profile: PetLifeProfile::default(),
```

- [ ] **Step 6: Add the default profile at production view-model construction**

In `src/commands/watch.rs`, add `life_profile` to the `WatchViewModel` literal:

```rust
        life_profile: crate::tui::life::PetLifeProfile::default(),
```

- [ ] **Step 7: Run focused build checks**

Run: `cargo test --lib tui::life`

Expected: PASS.

Run: `cargo test --lib build_watch_view_model_populates_progress_view`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/tui/life.rs src/tui/mod.rs src/tui/view_model.rs src/commands/watch.rs
git commit -m "feat(watch): add live pet profile contract"
```

---

### Task 2: Carry Token Bucket Detail Through Provider and Runtime

**Files:**
- Modify: `src/usage/provider.rs`
- Modify: `src/usage/ccusage.rs`
- Modify: `src/game/runtime.rs`
- Modify: `tests/usage_provider.rs`
- Modify: `tests/runtime_integration.rs`

- [ ] **Step 1: Write a failing provider test for raw bucket deltas**

In `tests/usage_provider.rs`, add:

```rust
#[test]
fn provider_deltas_carry_raw_token_bucket_detail() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider(Some("ccusage-ok.mjs"), None);

    let poll = provider.poll(&mut store).unwrap();
    let claude_delta = poll
        .deltas
        .iter()
        .find(|delta| delta.provider_surface == "claude-code")
        .expect("expected claude delta");
    let buckets = claude_delta
        .token_totals
        .expect("provider delta should include raw token bucket detail");

    assert!(
        buckets.uncached_input > 0 || buckets.output > 0 || buckets.cache_creation > 0,
        "expected non-empty bucket detail: {buckets:?}"
    );
}
```

- [ ] **Step 2: Run the provider test to verify it fails**

Run: `cargo test --test usage_provider provider_deltas_carry_raw_token_bucket_detail`

Expected: FAIL because `UsageDelta` does not have `token_totals`.

- [ ] **Step 3: Add bucket detail to `UsageDelta`**

In `src/usage/provider.rs`, add the import:

```rust
use crate::usage::normalize::RawTokenTotals;
```

Add the optional field:

```rust
    pub token_totals: Option<RawTokenTotals>,
```

- [ ] **Step 4: Fill bucket detail in `CcusageCommandProvider`**

In `src/usage/ccusage.rs`, in the `deltas.push(UsageDelta { ... })` literal, add:

```rust
                token_totals: Some(delta_totals),
```

Update any test-only `UsageDelta` literals to include:

```rust
                token_totals: None,
```

- [ ] **Step 5: Verify provider bucket detail is green**

Run: `cargo test --test usage_provider provider_deltas_carry_raw_token_bucket_detail`

Expected: PASS.

- [ ] **Step 6: Write a failing runtime test for apportioned token buckets**

In `tests/runtime_integration.rs`, add:

```rust
#[test]
fn staged_usage_apportions_token_buckets_across_smear_rows() {
    use glorp::game::calibration::CalibrationBaseline;
    use glorp::game::runtime::stage_usage_poll_deltas;
    use glorp::storage::usage_store::{ProviderCursorUpdate, UsageStore};
    use glorp::usage::normalize::RawTokenTotals;
    use glorp::usage::provider::{UsageDelta, UsagePollResult};
    use tempfile::tempdir;
    use time::OffsetDateTime;

    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let cursor_update = ProviderCursorUpdate {
        provider_surface: "claude-code".to_string(),
        cursor_key: "cursor".to_string(),
        cursor_value: "cursor-value".to_string(),
        provider_version: "test-provider".to_string(),
        parser_version: "test-parser".to_string(),
    };
    let poll = UsagePollResult {
        diagnostics: Vec::new(),
        total_effective_tokens: 12_000.0,
        deltas: vec![UsageDelta {
            provider_surface: "claude-code".to_string(),
            command: "ccusage".to_string(),
            effective_tokens: 12_000.0,
            confidence: "test".to_string(),
            period_start: now,
            observed_at: now,
            model: None,
            cursor_update,
            token_totals: Some(RawTokenTotals {
                uncached_input: 6_000,
                output: 3_000,
                cache_creation: 2_000,
                cache_read: 1_000,
                reasoning_output: 500,
            }),
        }],
    };

    let ids = stage_usage_poll_deltas(
        &mut store,
        &poll,
        CalibrationBaseline {
            tokens_per_xp: 100_000.0,
        },
        now,
    )
    .unwrap();
    assert!(ids.len() > 1, "test expects catchup smear to create multiple rows");

    let rows = store.unapplied_events(100).unwrap();
    let input_sum: f64 = rows.iter().map(|row| row.event.input_tokens).sum();
    let output_sum: f64 = rows.iter().map(|row| row.event.output_tokens).sum();
    let cache_creation_sum: f64 = rows.iter().map(|row| row.event.cache_creation_tokens).sum();
    let cache_read_sum: f64 = rows.iter().map(|row| row.event.cache_read_tokens).sum();
    let reasoning_sum: f64 = rows.iter().map(|row| row.event.reasoning_output_tokens).sum();

    assert!((input_sum - 6_000.0).abs() < 0.01);
    assert!((output_sum - 3_000.0).abs() < 0.01);
    assert!((cache_creation_sum - 2_000.0).abs() < 0.01);
    assert!((cache_read_sum - 1_000.0).abs() < 0.01);
    assert!((reasoning_sum - 500.0).abs() < 0.01);
}
```

- [ ] **Step 7: Run the runtime test to verify it fails**

Run: `cargo test --test runtime_integration staged_usage_apportions_token_buckets_across_smear_rows`

Expected: FAIL because staged rows currently write zero token buckets.

- [ ] **Step 8: Apportion bucket detail in runtime staging**

In `src/game/runtime.rs`, add a helper near `event_for_delta`:

```rust
fn scaled_token_bucket(total: Option<u64>, effective_share: f64, total_effective: f64) -> f64 {
    let Some(total) = total else {
        return 0.0;
    };
    if !effective_share.is_finite() || !total_effective.is_finite() || total_effective <= 0.0 {
        return 0.0;
    }
    (total as f64) * (effective_share / total_effective).clamp(0.0, 1.0)
}
```

In `stage_usage_poll_deltas`, compute each smeared row from the base event and set bucket fields after `effective_tokens` is known:

```rust
            let mut event = event_for_delta(delta, now);
            event.observed_at = now;
            event.bucket_at = bucket_at;
            event.effective_tokens = effective_tokens;
            if let Some(totals) = delta.token_totals {
                event.input_tokens = scaled_token_bucket(
                    Some(totals.uncached_input),
                    effective_tokens,
                    delta.effective_tokens,
                );
                event.output_tokens =
                    scaled_token_bucket(Some(totals.output), effective_tokens, delta.effective_tokens);
                event.cache_creation_tokens = scaled_token_bucket(
                    Some(totals.cache_creation),
                    effective_tokens,
                    delta.effective_tokens,
                );
                event.cache_read_tokens = scaled_token_bucket(
                    Some(totals.cache_read),
                    effective_tokens,
                    delta.effective_tokens,
                );
                event.reasoning_output_tokens = scaled_token_bucket(
                    Some(totals.reasoning_output),
                    effective_tokens,
                    delta.effective_tokens,
                );
            }
```

Keep `event_for_delta` defaults at `0.0`; missing detail stays absent-detail behavior.

- [ ] **Step 9: Verify runtime bucket apportionment**

Run: `cargo test --test runtime_integration staged_usage_apportions_token_buckets_across_smear_rows`

Expected: PASS.

- [ ] **Step 10: Verify provider/runtime affected tests**

Run: `cargo test --test usage_provider`

Expected: PASS.

Run: `cargo test --test runtime_integration`

Expected: PASS.

- [ ] **Step 11: Commit**

```bash
git add src/usage/provider.rs src/usage/ccusage.rs src/game/runtime.rs tests/usage_provider.rs tests/runtime_integration.rs
git commit -m "feat(runtime): preserve token bucket detail in applied usage"
```

---

### Task 3: Derive Applied Usage Signals From Deduped Applied Rows

**Files:**
- Modify: `src/tui/life.rs`
- Modify: `src/game/runtime.rs`

- [ ] **Step 1: Write failing signal-classification tests**

In `src/tui/life.rs`, extend the tests with:

```rust
    #[test]
    fn missing_detail_does_not_make_live_signal_non_live() {
        let now = time::macros::datetime!(2026-06-05 12:00 UTC);
        let signal = AppliedUsageSignal {
            applied_effective_tokens: 42_000.0,
            raw_effective_tokens: None,
            source_mix: None,
            token_shape: None,
            observed_at: now,
            elapsed_since_successful_poll: Duration::seconds(10),
            freshness: UsageSignalFreshness::Live,
        };

        assert_eq!(signal.freshness, UsageSignalFreshness::Live);
        assert!(signal.token_shape.is_none());
        assert!(signal.source_mix.is_none());
    }

    #[test]
    fn diagnostics_only_signal_is_non_live() {
        let now = time::macros::datetime!(2026-06-05 12:00 UTC);
        let signal = AppliedUsageSignal::diagnostics_only(now, Duration::seconds(10));

        assert_eq!(signal.freshness, UsageSignalFreshness::DiagnosticsOnly);
        assert_eq!(signal.applied_effective_tokens, 0.0);
        assert_eq!(signal.raw_effective_tokens, None);
        assert_eq!(signal.source_mix, None);
        assert_eq!(signal.token_shape, None);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib tui::life`

Expected: FAIL because `AppliedUsageSignal::diagnostics_only` does not exist.

- [ ] **Step 3: Add signal constructors and helpers**

In `src/tui/life.rs`, add:

```rust
impl AppliedUsageSignal {
    pub fn quiet(now: OffsetDateTime, elapsed_since_successful_poll: Duration) -> Self {
        Self {
            applied_effective_tokens: 0.0,
            raw_effective_tokens: None,
            source_mix: None,
            token_shape: None,
            observed_at: now,
            elapsed_since_successful_poll,
            freshness: UsageSignalFreshness::Live,
        }
    }

    pub fn diagnostics_only(now: OffsetDateTime, elapsed_since_successful_poll: Duration) -> Self {
        Self {
            freshness: UsageSignalFreshness::DiagnosticsOnly,
            ..Self::quiet(now, elapsed_since_successful_poll)
        }
    }

    pub fn can_burst(self) -> bool {
        self.freshness == UsageSignalFreshness::Live && self.applied_effective_tokens > 0.0
    }
}
```

- [ ] **Step 4: Verify signal helpers**

Run: `cargo test --lib tui::life`

Expected: PASS.

- [ ] **Step 5: Write a failing runtime signal test**

In `src/game/runtime.rs` test module, add a test that applies one Claude row and one Codex row, then asserts the update carries source and token-shape summaries. Use `NormalizedUsageEvent::for_test_at` and set `provider_surface`, `input_tokens`, `output_tokens`, `cache_read_tokens`, and `effective_tokens` on the rows before insertion.

The assertion shape should be:

```rust
        assert_eq!(update.applied_signal.freshness, UsageSignalFreshness::Live);
        assert_eq!(update.applied_signal.applied_effective_tokens, 30_000.0);
        assert_eq!(
            update.applied_signal.source_mix,
            Some(AppliedSourceMix {
                claude_effective_tokens: 10_000.0,
                codex_effective_tokens: 20_000.0,
            })
        );
        let shape = update.applied_signal.token_shape.expect("token shape");
        assert_eq!(shape.input_tokens, 5_000.0);
        assert_eq!(shape.output_tokens, 9_000.0);
        assert_eq!(shape.cache_read_tokens, 16_000.0);
```

- [ ] **Step 6: Run the runtime signal test to verify it fails**

Run: `cargo test --lib apply_unapplied_usage_returns_applied_signal_summary`

Expected: FAIL because `RuntimeUpdate` has no `applied_signal`.

- [ ] **Step 7: Add the signal to `RuntimeUpdate`**

In `src/game/runtime.rs`, import the life types:

```rust
use crate::tui::life::{
    AppliedSourceMix, AppliedUsageSignal, TokenShapeDelta, UsageSignalFreshness,
};
```

Extend `RuntimeUpdate`:

```rust
pub struct RuntimeUpdate {
    pub recent_effective_tokens: f64,
    pub applied_event_ids: Vec<i64>,
    pub applied_signal: AppliedUsageSignal,
}
```

Build the signal from `rows_to_apply` immediately before `Ok(RuntimeUpdate { ... })`:

```rust
let applied_signal = applied_signal_from_rows(&rows_to_apply, now, state.last_usage_poll_at);
```

Add helper functions:

```rust
fn applied_signal_from_rows(
    rows: &[crate::storage::usage_store::UsageLedgerRow],
    now: OffsetDateTime,
    previous_poll_at: Option<OffsetDateTime>,
) -> AppliedUsageSignal {
    let elapsed = previous_poll_at
        .map(|last| now - last)
        .unwrap_or_else(|| Duration::seconds(0));
    let applied_effective_tokens = rows
        .iter()
        .map(|row| row.event.effective_tokens.max(0.0))
        .sum::<f64>();

    let claude = rows
        .iter()
        .filter(|row| row.event.provider_surface.contains("claude"))
        .map(|row| row.event.effective_tokens.max(0.0))
        .sum::<f64>();
    let codex = rows
        .iter()
        .filter(|row| row.event.provider_surface.contains("codex"))
        .map(|row| row.event.effective_tokens.max(0.0))
        .sum::<f64>();
    let source_mix = if claude > 0.0 || codex > 0.0 {
        Some(AppliedSourceMix {
            claude_effective_tokens: claude,
            codex_effective_tokens: codex,
        })
    } else {
        None
    };

    let token_shape = token_shape_from_rows(rows);
    let freshness = if previous_poll_at.is_none() && applied_effective_tokens > 0.0 {
        UsageSignalFreshness::ColdStart
    } else {
        UsageSignalFreshness::Live
    };

    AppliedUsageSignal {
        applied_effective_tokens,
        raw_effective_tokens: None,
        source_mix,
        token_shape,
        observed_at: now,
        elapsed_since_successful_poll: elapsed,
        freshness,
    }
}

fn token_shape_from_rows(
    rows: &[crate::storage::usage_store::UsageLedgerRow],
) -> Option<TokenShapeDelta> {
    let shape = TokenShapeDelta {
        input_tokens: rows.iter().map(|row| row.event.input_tokens.max(0.0)).sum(),
        output_tokens: rows.iter().map(|row| row.event.output_tokens.max(0.0)).sum(),
        cache_creation_tokens: rows
            .iter()
            .map(|row| row.event.cache_creation_tokens.max(0.0))
            .sum(),
        cache_read_tokens: rows
            .iter()
            .map(|row| row.event.cache_read_tokens.max(0.0))
            .sum(),
        reasoning_output_tokens: rows
            .iter()
            .map(|row| row.event.reasoning_output_tokens.max(0.0))
            .sum(),
    };
    if shape.input_tokens > 0.0
        || shape.output_tokens > 0.0
        || shape.cache_creation_tokens > 0.0
        || shape.cache_read_tokens > 0.0
        || shape.reasoning_output_tokens > 0.0
    {
        Some(shape)
    } else {
        None
    }
}
```

Before updating `state.last_usage_poll_at`, capture the previous value:

```rust
let previous_poll_at = state.last_usage_poll_at;
```

Use `previous_poll_at` for the helper. This lets first-poll/cold-start classification happen without guessing from smear buckets.

- [ ] **Step 8: Verify runtime signal tests**

Run: `cargo test --lib apply_unapplied_usage_returns_applied_signal_summary`

Expected: PASS.

Run: `cargo test --lib runtime`

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src/tui/life.rs src/game/runtime.rs
git commit -m "feat(runtime): summarize applied usage for live pet profile"
```

---

### Task 4: Add `LifeSignalState` Normalization and Measurement

**Files:**
- Modify: `src/tui/life.rs`
- Create: `docs/superpowers/measurements/2026-06-05-glorp-life-normalization.md`

- [ ] **Step 1: Write failing `LifeSignalState` tests**

In `src/tui/life.rs`, add tests:

```rust
    fn live_signal(tokens: f64, elapsed: Duration, now: OffsetDateTime) -> AppliedUsageSignal {
        AppliedUsageSignal {
            applied_effective_tokens: tokens,
            raw_effective_tokens: Some(tokens),
            source_mix: None,
            token_shape: None,
            observed_at: now,
            elapsed_since_successful_poll: elapsed,
            freshness: UsageSignalFreshness::Live,
        }
    }

    #[test]
    fn life_signal_state_distinguishes_idle_warm_hot_and_cooling() {
        let start = time::macros::datetime!(2026-06-05 12:00 UTC);
        let mut state = LifeSignalState::default();

        let idle = state.observe(AppliedUsageSignal::quiet(start, Duration::seconds(10)), start);
        assert_eq!(idle.activity_level, 0.0);
        assert_eq!(idle.burst_level, 0.0);

        let warm = state.observe(
            live_signal(5_000.0, Duration::seconds(10), start + Duration::seconds(10)),
            start + Duration::seconds(10),
        );
        let hot = state.observe(
            live_signal(80_000.0, Duration::seconds(10), start + Duration::seconds(20)),
            start + Duration::seconds(20),
        );
        let cooling = state.observe(
            AppliedUsageSignal::quiet(start + Duration::seconds(90), Duration::seconds(70)),
            start + Duration::seconds(90),
        );

        assert!(warm.activity_level > idle.activity_level);
        assert!(hot.activity_level > warm.activity_level);
        assert!(cooling.activity_level < hot.activity_level);
        assert!(hot.burst_level > 0.0);
        assert_eq!(cooling.burst_level, 0.0);
    }

    #[test]
    fn non_live_signal_suppresses_burst_but_not_missing_detail() {
        let now = time::macros::datetime!(2026-06-05 12:00 UTC);
        let mut state = LifeSignalState::default();
        let cold = AppliedUsageSignal {
            freshness: UsageSignalFreshness::ColdStart,
            ..live_signal(80_000.0, Duration::seconds(10), now)
        };
        let cold_profile = state.observe(cold, now);
        assert_eq!(cold_profile.burst_level, 0.0);

        let live_missing_detail = live_signal(80_000.0, Duration::seconds(10), now + Duration::seconds(10));
        let live_profile = state.observe(live_missing_detail, now + Duration::seconds(10));
        assert!(live_profile.burst_level > 0.0);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib tui::life`

Expected: FAIL because `LifeSignalState` does not exist.

- [ ] **Step 3: Implement conservative normalization**

In `src/tui/life.rs`, add:

```rust
const DEFAULT_REFERENCE_TOKENS_PER_MINUTE: f64 = 60_000.0;
const MIN_REFERENCE_TOKENS_PER_MINUTE: f64 = 5_000.0;
const MAX_ACTIVITY_LEVEL: f32 = 2.0;
const EMA_ALPHA: f64 = 0.35;
const IDLE_DECAY_PER_MINUTE: f32 = 0.82;

#[derive(Debug, Clone)]
pub struct LifeSignalState {
    reference_tokens_per_minute: f64,
    ema_activity_level: f32,
    last_observed_at: Option<OffsetDateTime>,
}

impl Default for LifeSignalState {
    fn default() -> Self {
        Self {
            reference_tokens_per_minute: DEFAULT_REFERENCE_TOKENS_PER_MINUTE,
            ema_activity_level: 0.0,
            last_observed_at: None,
        }
    }
}

impl LifeSignalState {
    pub fn observe(&mut self, signal: AppliedUsageSignal, now: OffsetDateTime) -> PetLifeProfile {
        let elapsed_secs = signal
            .elapsed_since_successful_poll
            .whole_seconds()
            .max(1) as f64;
        let tokens_per_minute = signal.applied_effective_tokens.max(0.0) / elapsed_secs * 60.0;
        if signal.freshness == UsageSignalFreshness::Live && tokens_per_minute > 0.0 {
            self.reference_tokens_per_minute = ((1.0 - EMA_ALPHA) * self.reference_tokens_per_minute
                + EMA_ALPHA * tokens_per_minute)
                .clamp(MIN_REFERENCE_TOKENS_PER_MINUTE, DEFAULT_REFERENCE_TOKENS_PER_MINUTE * 20.0);
        }
        let target = activity_from_rate(tokens_per_minute, self.reference_tokens_per_minute);
        if signal.freshness == UsageSignalFreshness::Live {
            self.ema_activity_level =
                ((1.0 - EMA_ALPHA as f32) * self.ema_activity_level + EMA_ALPHA as f32 * target)
                    .clamp(0.0, MAX_ACTIVITY_LEVEL);
        } else {
            self.ema_activity_level = decay_activity(self.ema_activity_level, signal.elapsed_since_successful_poll);
        }
        if signal.applied_effective_tokens == 0.0 {
            self.ema_activity_level = decay_activity(self.ema_activity_level, signal.elapsed_since_successful_poll);
        }
        self.last_observed_at = Some(now);

        let burst_level = if signal.can_burst() {
            activity_from_rate(tokens_per_minute, self.reference_tokens_per_minute)
        } else {
            0.0
        };

        PetLifeProfile {
            activity_level: self.ema_activity_level,
            burst_level,
            source_accent: classify_source_accent(signal.source_mix),
            work_weather: classify_work_weather(signal.token_shape),
            prop_reactions: Vec::new(),
            idle: IdleLifeState {
                idle_minutes: idle_minutes(signal.elapsed_since_successful_poll),
                is_recently_active: signal.applied_effective_tokens > 0.0,
            },
            calm_mode: false,
        }
    }
}

fn activity_from_rate(tokens_per_minute: f64, reference_tokens_per_minute: f64) -> f32 {
    if !tokens_per_minute.is_finite() || tokens_per_minute <= 0.0 {
        return 0.0;
    }
    let reference = reference_tokens_per_minute
        .max(MIN_REFERENCE_TOKENS_PER_MINUTE)
        .max(1.0);
    let ratio = tokens_per_minute / reference;
    (2.0 * ratio / (1.0 + ratio)) as f32
}

fn decay_activity(current: f32, elapsed: Duration) -> f32 {
    let minutes = (elapsed.whole_seconds().max(0) as f32) / 60.0;
    (current * IDLE_DECAY_PER_MINUTE.powf(minutes)).clamp(0.0, MAX_ACTIVITY_LEVEL)
}

fn idle_minutes(elapsed: Duration) -> u32 {
    elapsed.whole_minutes().max(0) as u32
}
```

Add basic classifiers used above:

```rust
pub fn classify_source_accent(source_mix: Option<AppliedSourceMix>) -> Option<SourceAccent> {
    let mix = source_mix?;
    let total = mix.claude_effective_tokens + mix.codex_effective_tokens;
    if total <= 0.0 || !total.is_finite() {
        return None;
    }
    let claude_share = mix.claude_effective_tokens / total;
    if (0.4..=0.6).contains(&claude_share) {
        Some(SourceAccent::Balanced)
    } else if claude_share > 0.6 {
        Some(SourceAccent::Claude)
    } else {
        Some(SourceAccent::Codex)
    }
}

pub fn classify_work_weather(shape: Option<TokenShapeDelta>) -> WorkWeather {
    let Some(shape) = shape else {
        return WorkWeather::Clear;
    };
    let total = shape.input_tokens
        + shape.output_tokens
        + shape.cache_creation_tokens
        + shape.cache_read_tokens
        + shape.reasoning_output_tokens;
    if total <= 0.0 || !total.is_finite() {
        return WorkWeather::Clear;
    }
    let cache = (shape.cache_creation_tokens + shape.cache_read_tokens) / total;
    let output = shape.output_tokens / total;
    let reasoning = shape.reasoning_output_tokens / total;
    if cache >= 0.55 {
        WorkWeather::CacheMist
    } else if output >= 0.45 {
        WorkWeather::OutputSparks
    } else if reasoning >= 0.30 {
        WorkWeather::ReasoningPulse
    } else if cache >= 0.25 || output >= 0.25 || reasoning >= 0.15 {
        WorkWeather::Mixed
    } else {
        WorkWeather::Clear
    }
}
```

- [ ] **Step 4: Verify normalization tests pass**

Run: `cargo test --lib tui::life`

Expected: PASS.

- [ ] **Step 5: Add measurement note**

Create `docs/superpowers/measurements/2026-06-05-glorp-life-normalization.md`:

```markdown
# Glorp Life Normalization Measurement

Date: 2026-06-05

The first Branch 2 implementation uses a session-local reference pace:

- default reference: 60,000 effective tokens/minute
- minimum reference: 5,000 effective tokens/minute
- EMA alpha: 0.35
- display range: 0.0..=2.0
- idle decay: multiply by 0.82 per idle minute

The curve is `2 * ratio / (1 + ratio)`, where `ratio = current_tokens_per_minute / reference`.

Representative outputs:

| Signal | Tokens | Elapsed | Pace | Approx level |
| --- | ---: | ---: | ---: | ---: |
| idle | 0 | 10s | 0/min | 0.00 |
| warm | 5,000 | 10s | 30,000/min | 0.67 |
| hot | 80,000 | 10s | 480,000/min | 1.78 |
| very hot | 200,000 | 10s | 1,200,000/min | 1.90 |

This leaves visible room between warm/hot/very-hot without requiring persistent calibration. Cold-start, backfill, and diagnostics-only signals can update activity slowly if desired, but they do not create burst.
```

- [ ] **Step 6: Commit**

```bash
git add src/tui/life.rs docs/superpowers/measurements/2026-06-05-glorp-life-normalization.md
git commit -m "feat(watch): normalize live pet activity"
```

---

### Task 5: Thread Poll Envelopes Through Watch and Menubar

**Files:**
- Modify: `src/tui/app.rs`
- Modify: `src/commands/watch.rs`
- Modify: `src/menubar/app.rs`
- Modify: `src/tui/view_model.rs`

- [ ] **Step 1: Write a failing watch-app test for profile stamping**

In `src/tui/app.rs` tests, add a test poller that returns a live signal and assert the resulting VM has non-zero activity after refresh:

```rust
struct SignalPoller {
    signal: crate::tui::life::AppliedUsageSignal,
}

impl WatchUsagePoller for SignalPoller {
    fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchPollResult> {
        Ok(WatchPollResult {
            vm: current.clone(),
            applied_signal: self.signal,
        })
    }
}

#[test]
fn refresh_stamps_life_profile_from_applied_signal() {
    let now = time::macros::datetime!(2026-06-05 12:00 UTC);
    let vm = WatchViewModel::fixture();
    let signal = crate::tui::life::AppliedUsageSignal {
        applied_effective_tokens: 80_000.0,
        raw_effective_tokens: Some(80_000.0),
        source_mix: None,
        token_shape: None,
        observed_at: now,
        elapsed_since_successful_poll: time::Duration::seconds(10),
        freshness: crate::tui::life::UsageSignalFreshness::Live,
    };
    let mut app = WatchApp::with_poll_callback(
        vm,
        Default::default(),
        Box::new(SignalPoller { signal }),
    );

    let refreshed = app.refresh_for_test().unwrap();

    assert!(refreshed.life_profile.activity_level > 0.0);
    assert!(refreshed.life_profile.burst_level > 0.0);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib refresh_stamps_life_profile_from_applied_signal`

Expected: FAIL because `WatchPollResult` and profile stamping do not exist.

- [ ] **Step 3: Add `WatchPollResult` and update the poll trait**

In `src/tui/app.rs`, add:

```rust
#[derive(Debug, Clone)]
pub struct WatchPollResult {
    pub vm: WatchViewModel,
    pub applied_signal: crate::tui::life::AppliedUsageSignal,
}
```

Change:

```rust
pub trait WatchUsagePoller: Send {
    fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchViewModel>;
}
```

to:

```rust
pub trait WatchUsagePoller: Send {
    fn poll_usage(&mut self, current: &WatchViewModel) -> Result<WatchPollResult>;
}
```

In `WatchApp::with_poll_callback`, update the result channel type:

```rust
let (result_tx, result_rx) = mpsc::channel::<Result<WatchPollResult>>();
```

Update `NoopWatchPoller`:

```rust
Ok(WatchPollResult {
    vm: current.clone(),
    applied_signal: crate::tui::life::AppliedUsageSignal::quiet(
        time::OffsetDateTime::now_utc(),
        time::Duration::seconds(0),
    ),
})
```

- [ ] **Step 4: Add `LifeSignalState` to `WatchApp`**

In `WatchApp`, add:

```rust
life_signal_state: crate::tui::life::LifeSignalState,
```

Initialize it in constructors:

```rust
life_signal_state: crate::tui::life::LifeSignalState::default(),
```

In `try_collect_poll_result`, replace:

```rust
                self.vm = result?;
```

with:

```rust
                let mut result = result?;
                let profile = self
                    .life_signal_state
                    .observe(result.applied_signal, time::OffsetDateTime::now_utc());
                result.vm.life_profile = profile;
                self.vm = result.vm;
```

- [ ] **Step 5: Update the real watch poller**

In `src/commands/watch.rs`, import `WatchPollResult`:

```rust
        app::{WatchApp, WatchPollResult, WatchUsagePoller},
```

Change `RealWatchPoller::poll_usage` to return `Result<WatchPollResult>`. On error, return a quiet signal:

```rust
return Ok(WatchPollResult {
    vm,
    applied_signal: crate::tui::life::AppliedUsageSignal::diagnostics_only(
        OffsetDateTime::now_utc(),
        Duration::seconds(0),
    ),
});
```

Change `poll_usage_and_apply` to return the state and signal:

```rust
pub(crate) struct PollUsageOutcome {
    pub state: PetState,
    pub applied_signal: crate::tui::life::AppliedUsageSignal,
}

pub(crate) fn poll_usage_and_apply(
    state_store: &StateStore,
    usage_db: &Path,
    config_file: &Path,
) -> Result<Option<PollUsageOutcome>> {
```

When updates are applied, use `update.applied_signal`. When there is no pet, return `Ok(None)`. When poll has diagnostics and no deltas, return current state plus `AppliedUsageSignal::diagnostics_only`.

Build the VM and return:

```rust
Ok(WatchPollResult {
    vm: build_watch_view_model(&outcome.state, &self.usage_db)?,
    applied_signal: outcome.applied_signal,
})
```

- [ ] **Step 6: Update menubar worker types**

In `src/menubar/app.rs`, change `PollResult` to include:

```rust
applied_signal: crate::tui::life::AppliedUsageSignal,
```

Update `spawn_poll_worker` to call the new `poll_usage_and_apply`, build the VM from `outcome.state`, and send `applied_signal`.

In the UI tick code where poll results are received, add a menubar-local `LifeSignalState` to app state and stamp `vm.life_profile` before rendering/writing blocks.

- [ ] **Step 7: Verify watch profile stamping**

Run: `cargo test --lib refresh_stamps_life_profile_from_applied_signal`

Expected: PASS.

Run: `cargo test --lib tui::app`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/tui/app.rs src/commands/watch.rs src/menubar/app.rs src/tui/view_model.rs
git commit -m "feat(watch): thread live usage signals through pollers"
```

---

### Task 6: Add Preview Lab Liveliness Fixtures and Proof Tests

**Files:**
- Modify: `src/dev_preview/watch.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Modify: `tests/dev_preview.rs`

- [ ] **Step 1: Write failing manifest/fixture test**

In `tests/dev_preview.rs`, add:

```rust
#[test]
fn dev_preview_watch_includes_liveliness_profile_inputs() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let manifest = run.manifest();
    for id in [
        "watch-liveliness-s6-idle-dawn",
        "watch-liveliness-s6-warm-midday",
        "watch-liveliness-s6-hot-midday",
        "watch-liveliness-s6-cooling-evening",
        "watch-liveliness-compact-s6-hot",
        "watch-liveliness-flat-s6-hot",
        "watch-liveliness-calm-mode-s6-hot",
    ] {
        let scenario = manifest["scenarios"]
            .as_array()
            .unwrap()
            .iter()
            .find(|scenario| scenario["id"] == id)
            .unwrap_or_else(|| panic!("missing scenario {id}"));
        assert!(scenario["inputs"]["life_profile"]["activity_level"].is_number());
        assert!(scenario["inputs"]["life_profile"]["burst_level"].is_number());
        assert!(scenario["inputs"]["life_profile"]["freshness"].is_string());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --test dev_preview dev_preview_watch_includes_liveliness_profile_inputs`

Expected: FAIL because the liveliness scenarios do not exist.

- [ ] **Step 3: Add deterministic profile fixture helpers**

In `src/dev_preview/watch.rs`, add a fixture profile type:

```rust
#[derive(Clone)]
pub(crate) struct WatchLifeFixture {
    pub profile: crate::tui::life::PetLifeProfile,
    pub color_capability: crate::tui::style::ColorCapability,
}
```

Add a helper:

```rust
fn render_watch_frame_with_life(
    ctx: &PreviewRenderContext,
    scratch_dir: &Path,
    fixture: WatchFrameFixture<'_>,
    life: WatchLifeFixture,
) -> Result<PreviewFrame> {
    let usage_path = scratch_dir.join(format!("{}.sqlite", fixture.id));
    seed_usage_store(&usage_path, fixture.now)?;
    let render = RenderContext::with_clock(life.color_capability, WatchClock::fixed(fixture.now));
    let mut vm = build_watch_view_model_at(fixture.state, &usage_path, fixture.now, UtcOffset::UTC)?;
    vm.life_profile = life.profile;
    let layout = layout_watch_with_context(Rect::new(0, 0, fixture.width, fixture.height), &vm, &render);

    let mut terminal = Terminal::new(TestBackend::new(fixture.width, fixture.height))?;
    terminal.draw(|frame| {
        render_watch_frame_with_layout(frame, &vm, &render, &layout);
    })?;

    let mut frame = frame_from_buffer(fixture.id, fixture.title, terminal.backend().buffer());
    frame.layout = Some(preview_layout(fixture.id, &layout));
    Ok(frame)
}
```

Create helper profiles:

```rust
fn fixture_profile(activity_level: f32, burst_level: f32) -> crate::tui::life::PetLifeProfile {
    crate::tui::life::PetLifeProfile {
        activity_level,
        burst_level,
        ..Default::default()
    }
}
```

Extend `watch_frames` with the seven liveliness frames. Use S6 Crystal or Fuzz state, fixed times, 120x32 for normal, 72x24 for compact, `ColorCapability::Flat` for flat, and `calm_mode = true` for calm-mode.

- [ ] **Step 4: Add manifest inputs for liveliness frames**

In `src/dev_preview/scenarios.rs`, in `scenario_for_frame`, add a match arm for `id if id.starts_with("watch-liveliness-")` that returns `PreviewScenarioKind::Watch` and inputs:

```rust
BTreeMap::from([
    ("life_profile".to_string(), json!(life_profile_inputs_for_frame(id))),
    ("terminal_width".to_string(), json!(frame.width)),
    ("terminal_height".to_string(), json!(frame.height)),
])
```

Add:

```rust
fn life_profile_inputs_for_frame(id: &str) -> serde_json::Value {
    match id {
        "watch-liveliness-s6-idle-dawn" => json!({
            "activity_level": 0.0,
            "burst_level": 0.0,
            "source_accent": null,
            "weather": "clear",
            "stage": "S6",
            "species": "crystal",
            "prop_reactions": [],
            "color_capability": "truecolor",
            "calm_mode": false,
            "freshness": "Live"
        }),
        "watch-liveliness-s6-hot-midday" => json!({
            "activity_level": 1.7,
            "burst_level": 1.2,
            "source_accent": "balanced",
            "weather": "output-sparks",
            "stage": "S6",
            "species": "crystal",
            "prop_reactions": ["codex_signal_lamp", "heavy_session_planter"],
            "color_capability": "truecolor",
            "calm_mode": false,
            "freshness": "Live"
        }),
        _ => json!({
            "activity_level": 0.8,
            "burst_level": 0.2,
            "source_accent": null,
            "weather": "clear",
            "stage": "S6",
            "species": "crystal",
            "prop_reactions": [],
            "color_capability": "truecolor",
            "calm_mode": id.contains("calm-mode"),
            "freshness": "Live"
        }),
    }
}
```

- [ ] **Step 5: Verify manifest fixture test passes**

Run: `cargo test --test dev_preview dev_preview_watch_includes_liveliness_profile_inputs`

Expected: PASS.

- [ ] **Step 6: Write targeted cell-diff test**

In `tests/dev_preview.rs`, add helpers to read cells and layout, then add:

```rust
#[test]
fn dev_preview_liveliness_changes_pet_scene_cells_not_only_text() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let idle = read_cells(run.out.join("frames/watch-liveliness-s6-idle-dawn.cells.json"));
    let hot = read_cells(run.out.join("frames/watch-liveliness-s6-hot-midday.cells.json"));
    let hot_layout = read_layout(run.out.join("frames/watch-liveliness-s6-hot-midday.layout.json"));
    let habitat = target_rect(&hot_layout, "watch.pet.habitat");
    let changed_in_habitat = changed_cells_inside(&idle, &hot, habitat);

    assert!(
        changed_in_habitat >= 8,
        "expected at least 8 pet/habitat cell changes, got {changed_in_habitat}"
    );
}
```

Expected helper shape:

```rust
fn changed_cells_inside(a: &serde_json::Value, b: &serde_json::Value, rect: RectJson) -> usize {
    let a_cells = a["cells"].as_array().unwrap();
    let b_cells = b["cells"].as_array().unwrap();
    a_cells
        .iter()
        .zip(b_cells)
        .filter(|(left, right)| {
            let x = right["x"].as_u64().unwrap() as u16;
            let y = right["y"].as_u64().unwrap() as u16;
            x >= rect.x
                && x < rect.x + rect.width
                && y >= rect.y
                && y < rect.y + rect.height
                && (left["symbol"] != right["symbol"] || left["fg"] != right["fg"])
        })
        .count()
}
```

- [ ] **Step 7: Run the targeted cell-diff test to verify it fails**

Run: `cargo test --test dev_preview dev_preview_liveliness_changes_pet_scene_cells_not_only_text`

Expected: FAIL until visual consumers are added in Task 7. Keep the failing assertion committed only if the execution workflow supports RED commits. If not, leave this test unstaged until Task 7 makes it pass.

- [ ] **Step 8: Commit fixture scaffolding**

```bash
git add src/dev_preview/watch.rs src/dev_preview/scenarios.rs tests/dev_preview.rs
git commit -m "test(preview): add liveliness fixture proof contract"
```

---

### Task 7: Render Profile-Driven Pet Panel Liveliness

**Files:**
- Modify: `src/tui/panels/pet.rs`
- Modify: `src/tui/life.rs`
- Modify: `tests/dev_preview.rs`

- [ ] **Step 1: Write failing unit tests for visual helper caps**

In `src/tui/panels/pet.rs` tests, add:

```rust
#[test]
fn activity_glyph_budget_caps_compact_hot_state() {
    let profile = crate::tui::life::PetLifeProfile {
        activity_level: 2.0,
        burst_level: 1.5,
        ..Default::default()
    };

    assert_eq!(activity_glyph_budget(&profile, true), 3);
    assert_eq!(activity_glyph_budget(&profile, false), 10);
}

#[test]
fn activity_style_lift_is_clamped_and_flat_safe() {
    let style = activity_lift_style(
        ratatui::style::Style::default().fg(ratatui::style::Color::Rgb(100, 100, 100)),
        2.0,
        crate::tui::style::ColorCapability::Truecolor,
    );
    assert_ne!(style, ratatui::style::Style::default().fg(ratatui::style::Color::Rgb(100, 100, 100)));

    let flat = activity_lift_style(
        ratatui::style::Style::default().fg(ratatui::style::Color::Rgb(100, 100, 100)),
        2.0,
        crate::tui::style::ColorCapability::Flat,
    );
    assert_eq!(flat, ratatui::style::Style::default().fg(ratatui::style::Color::Rgb(100, 100, 100)));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib tui::panels::pet`

Expected: FAIL because helpers do not exist.

- [ ] **Step 3: Implement helper functions**

In `src/tui/panels/pet.rs`, add:

```rust
fn activity_glyph_budget(profile: &crate::tui::life::PetLifeProfile, compact: bool) -> usize {
    if profile.calm_mode {
        return 0;
    }
    let base = if compact { 3.0 } else { 10.0 };
    ((profile.activity_level.clamp(0.0, 2.0) / 2.0) * base).round() as usize
}

fn activity_lift_style(
    style: Style,
    activity_level: f32,
    color_capability: crate::tui::style::ColorCapability,
) -> Style {
    if matches!(color_capability, crate::tui::style::ColorCapability::Flat) {
        return style;
    }
    let lift = (activity_level.clamp(0.0, 2.0) * 22.0) as u8;
    match style.fg {
        Some(Color::Rgb(r, g, b)) => style.fg(Color::Rgb(
            r.saturating_add(lift),
            g.saturating_add(lift),
            b.saturating_add(lift),
        )),
        _ => style,
    }
}
```

- [ ] **Step 4: Apply activity style and glyphs in `PetPanel`**

In `render_pet_inside`, before writing each styled segment, wrap the style:

```rust
let style = activity_lift_style(style, vm.life_profile.activity_level, ctx.color_capability);
```

In the ambient glyph pass, after base ambient glyphs are rendered, add deterministic activity glyphs inside `scene.habitat` and outside exclusions:

```rust
let compact = area.width <= 72 || area.height <= 24;
let extra_count = activity_glyph_budget(&vm.life_profile, compact);
for g in activity_glyphs_for(&vm.life_profile, species, scene.habitat, &ambient_exclusions, now, extra_count) {
    let cell = &mut buf[(g.col, g.row)];
    cell.set_char(g.glyph);
    cell.set_style(Style::default().fg(g.color));
}
```

Add `activity_glyphs_for` with a deterministic seed from species, minute, `activity_level`, and `work_weather`; use `['✦', '✧', '·', '*']` for hot states and keep Flat/calm-mode returning empty.

- [ ] **Step 5: Verify pet-panel helper tests**

Run: `cargo test --lib tui::panels::pet`

Expected: PASS.

- [ ] **Step 6: Run Preview Lab targeted cell-diff test**

Run: `cargo test --test dev_preview dev_preview_liveliness_changes_pet_scene_cells_not_only_text`

Expected: PASS.

- [ ] **Step 7: Verify compact and Flat previews**

Run: `cargo test --test dev_preview`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/tui/panels/pet.rs tests/dev_preview.rs
git commit -m "feat(tui): render live pet panel activity"
```

---

### Task 8: Add Source Accents, Weather, and Earned Prop Reactions

**Files:**
- Modify: `src/tui/life.rs`
- Modify: `src/tui/component/habitat_props.rs`
- Modify: `src/tui/panels/pet.rs`

- [ ] **Step 1: Write classifier tests**

In `src/tui/life.rs`, add tests:

```rust
#[test]
fn source_accent_handles_claude_codex_balanced_and_missing() {
    assert_eq!(classify_source_accent(None), None);
    assert_eq!(
        classify_source_accent(Some(AppliedSourceMix {
            claude_effective_tokens: 9.0,
            codex_effective_tokens: 1.0,
        })),
        Some(SourceAccent::Claude)
    );
    assert_eq!(
        classify_source_accent(Some(AppliedSourceMix {
            claude_effective_tokens: 1.0,
            codex_effective_tokens: 9.0,
        })),
        Some(SourceAccent::Codex)
    );
    assert_eq!(
        classify_source_accent(Some(AppliedSourceMix {
            claude_effective_tokens: 5.0,
            codex_effective_tokens: 5.0,
        })),
        Some(SourceAccent::Balanced)
    );
}

#[test]
fn weather_classifier_handles_missing_and_bucket_mix() {
    assert_eq!(classify_work_weather(None), WorkWeather::Clear);
    assert_eq!(
        classify_work_weather(Some(TokenShapeDelta {
            input_tokens: 10.0,
            output_tokens: 10.0,
            cache_creation_tokens: 10.0,
            cache_read_tokens: 70.0,
            reasoning_output_tokens: 0.0,
        })),
        WorkWeather::CacheMist
    );
    assert_eq!(
        classify_work_weather(Some(TokenShapeDelta {
            input_tokens: 10.0,
            output_tokens: 80.0,
            cache_creation_tokens: 0.0,
            cache_read_tokens: 0.0,
            reasoning_output_tokens: 10.0,
        })),
        WorkWeather::OutputSparks
    );
}
```

- [ ] **Step 2: Run classifier tests**

Run: `cargo test --lib tui::life`

Expected: PASS if Task 4 added the classifiers; otherwise FAIL then implement exactly as Task 4 specified.

- [ ] **Step 3: Write prop reaction test**

In `src/tui/life.rs`, add:

```rust
#[test]
fn prop_reactions_target_only_earned_visible_props() {
    let earned = vec![
        HabitatPropId::new(crate::game::habitat::CODEX_SIGNAL_LAMP),
        HabitatPropId::new(crate::game::habitat::HEAVY_SESSION_PLANTER),
    ];
    let profile = build_prop_reactions(
        PetLifeProfile {
            activity_level: 1.5,
            burst_level: 1.0,
            source_accent: Some(SourceAccent::Codex),
            ..Default::default()
        },
        &earned,
        true,
    );

    assert!(profile
        .prop_reactions
        .iter()
        .any(|reaction| reaction.prop_id.as_str() == "codex_signal_lamp"));
    assert!(!profile
        .prop_reactions
        .iter()
        .any(|reaction| reaction.kind == PropReactionKind::Orbit));
}
```

- [ ] **Step 4: Run prop reaction test to verify it fails**

Run: `cargo test --lib tui::life::tests::prop_reactions_target_only_earned_visible_props`

Expected: FAIL because `build_prop_reactions` does not exist.

- [ ] **Step 5: Implement prop reaction builder**

In `src/tui/life.rs`, add:

```rust
pub fn build_prop_reactions(
    mut profile: PetLifeProfile,
    earned: &[HabitatPropId],
    compact: bool,
) -> PetLifeProfile {
    profile.prop_reactions = earned
        .iter()
        .filter_map(|id| {
            let reaction = match (id.as_str(), profile.source_accent) {
                (crate::game::habitat::CODEX_SIGNAL_LAMP, Some(SourceAccent::Codex | SourceAccent::Balanced)) => {
                    Some(PropReactionKind::Glow)
                }
                (crate::game::habitat::HEAVY_SESSION_PLANTER, _) if profile.burst_level > 0.5 => {
                    Some(PropReactionKind::Bloom)
                }
                _ => None,
            }?;
            let reaction_kind = if compact && matches!(reaction, PropReactionKind::Orbit) {
                PropReactionKind::Glow
            } else {
                reaction
            };
            Some(PropReaction {
                prop_id: id.clone(),
                intensity: profile.activity_level.clamp(0.0, 2.0) / 2.0,
                kind: reaction_kind,
            })
        })
        .collect();
    profile
}
```

- [ ] **Step 6: Apply reactions in `habitat_props_for`**

Add a `life_profile: &PetLifeProfile` argument only if the call sites can thread it cleanly; otherwise pass `&vm.life_profile` from `PetPanel` to a small wrapper inside `PetPanel` that adjusts the returned cells. Keep the public `habitat_props_for` stable if the call-site churn is larger than the effect.

For each matching `HabitatPropCell`, apply a style lift:

```rust
fn apply_prop_reaction_style(
    style: Style,
    reaction: Option<&crate::tui::life::PropReaction>,
    color_capability: crate::tui::style::ColorCapability,
) -> Style {
    if matches!(color_capability, crate::tui::style::ColorCapability::Flat) {
        return style;
    }
    let Some(reaction) = reaction else {
        return style;
    };
    let lift = (reaction.intensity.clamp(0.0, 1.0) * 35.0) as u8;
    match style.fg {
        Some(Color::Rgb(r, g, b)) => style.fg(Color::Rgb(
            r.saturating_add(lift),
            g.saturating_add(lift),
            b.saturating_add(lift),
        )),
        _ => style,
    }
}
```

- [ ] **Step 7: Verify life and prop tests**

Run: `cargo test --lib tui::life`

Expected: PASS.

Run: `cargo test --lib tui::component::habitat_props`

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/tui/life.rs src/tui/component/habitat_props.rs src/tui/panels/pet.rs
git commit -m "feat(tui): add source weather and prop reactions"
```

---

### Task 9: Repoint Speech and Feed to the Live Profile

**Files:**
- Modify: `src/pet/speech.rs`
- Modify: `src/pet/activity.rs`
- Modify: `src/commands/watch.rs`

- [ ] **Step 1: Write speech profile tests**

In `src/pet/speech.rs`, add:

```rust
#[test]
fn speech_uses_profile_burst_for_munch_reaction() {
    let visible = datetime!(2026-05-11 12:00 UTC);
    let profile = crate::tui::life::PetLifeProfile {
        burst_level: 1.0,
        ..Default::default()
    };

    let speech = current_pet_speech_for_profile(Mood::Content, &profile, visible).unwrap();

    let munch_phrases = ["yum!", "more!", "tasty!", "delicious", "*chomp*"];
    assert!(munch_phrases.contains(&speech.as_str()));
}

#[test]
fn speech_does_not_fake_munch_when_profile_is_idle() {
    let visible = datetime!(2026-05-11 12:00 UTC);
    let profile = crate::tui::life::PetLifeProfile::default();

    let speech = current_pet_speech_for_profile(Mood::Content, &profile, visible).unwrap();

    let munch_phrases = ["yum!", "more!", "tasty!", "delicious", "*chomp*"];
    assert!(!munch_phrases.contains(&speech.as_str()));
}
```

- [ ] **Step 2: Run speech tests to verify they fail**

Run: `cargo test --lib pet::speech`

Expected: FAIL because `current_pet_speech_for_profile` does not exist.

- [ ] **Step 3: Implement profile speech wrapper**

In `src/pet/speech.rs`, add:

```rust
pub fn current_pet_speech_for_profile(
    mood: Mood,
    profile: &crate::tui::life::PetLifeProfile,
    now: OffsetDateTime,
) -> Option<String> {
    let cycle_pos = now.unix_timestamp().rem_euclid(SPEECH_CYCLE_SECS);
    if cycle_pos >= SPEECH_VISIBLE_SECS {
        return None;
    }
    if profile.burst_level >= 0.35 || profile.activity_level >= 1.25 {
        return Some(pick_munch_phrase(now));
    }
    Some(mood_phrase(mood, now))
}
```

Keep the existing `current_pet_speech` function for compatibility until all callers are moved.

- [ ] **Step 4: Move watch speech call to profile after stamping**

In `WatchApp` profile stamping, after `result.vm.life_profile = profile`, update:

```rust
result.vm.current_speech = crate::pet::speech::current_pet_speech_for_profile(
    result.vm.pet_render.mood,
    &result.vm.life_profile,
    time::OffsetDateTime::now_utc(),
);
```

If `WatchApp` cannot access mood as `Mood`, add `mood` to `PetRenderModel` is already present; use `result.vm.pet_render.mood`.

- [ ] **Step 5: Write feed profile tests**

In `src/pet/activity.rs`, add tests that call a new `derive_profile_pet_activities` with hot and idle profiles:

```rust
#[test]
fn profile_activity_adds_sparse_live_line_for_hot_profile() {
    let now = datetime!(2026-05-11 12:00 UTC);
    let profile = crate::tui::life::PetLifeProfile {
        activity_level: 1.5,
        burst_level: 0.8,
        ..Default::default()
    };
    let acts = derive_profile_pet_activities("luxopal", Species::Crystal, Mood::Happy, &profile, now);

    assert_eq!(acts.len(), 1);
    assert_eq!(acts[0].kind, LogKind::PetActivity);
    assert!(acts[0].text.contains("luxopal"));
}

#[test]
fn profile_activity_stays_silent_for_quiet_recent_profile() {
    let now = datetime!(2026-05-11 12:00 UTC);
    let profile = crate::tui::life::PetLifeProfile::default();
    let acts = derive_profile_pet_activities("luxopal", Species::Crystal, Mood::Happy, &profile, now);

    assert!(acts.is_empty());
}
```

- [ ] **Step 6: Run feed tests to verify they fail**

Run: `cargo test --lib pet::activity`

Expected: FAIL because `derive_profile_pet_activities` does not exist.

- [ ] **Step 7: Implement profile activity feed lines**

In `src/pet/activity.rs`, add:

```rust
pub fn derive_profile_pet_activities(
    pet_name: &str,
    species: Species,
    mood: Mood,
    profile: &crate::tui::life::PetLifeProfile,
    now: OffsetDateTime,
) -> Vec<EventView> {
    if profile.burst_level < 0.35 && profile.activity_level < 1.25 {
        return Vec::new();
    }
    let verb = match (profile.work_weather, species, mood) {
        (crate::tui::life::WorkWeather::CacheMist, _, _) => "is glowing through cached light",
        (crate::tui::life::WorkWeather::OutputSparks, _, _) => "sparked at the edges",
        (crate::tui::life::WorkWeather::ReasoningPulse, _, _) => "pulsed thoughtfully",
        (_, Species::Crystal, _) => "rang softly with work",
        (_, _, Mood::Sleepy) => "perked up",
        _ => "brightened",
    };
    vec![EventView {
        timestamp: format_hhmm(now),
        kind: LogKind::PetActivity,
        text: format!("{pet_name} {verb}"),
    }]
}
```

Merge these profile activities with existing `pet_activities` in the watch path after profile stamping, or in the next `build_watch_view_model_at` call when a profile is available. Keep a cap of one profile line per poll.

- [ ] **Step 8: Verify speech/feed tests**

Run:

```bash
cargo test --lib pet::speech
cargo test --lib pet::activity
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src/pet/speech.rs src/pet/activity.rs src/commands/watch.rs src/tui/app.rs
git commit -m "feat(pet): drive speech and feed from live profile"
```

---

### Task 10: Add Constrained Menubar Profile Accents

**Files:**
- Modify: `src/menubar/render.rs`
- Modify: `src/menubar/app.rs`

- [ ] **Step 1: Write menubar color mapping test**

In `src/menubar/render.rs` tests, add:

```rust
#[test]
fn menubar_profile_accent_is_poll_bound_and_bmp_safe() {
    let mut vm = WatchViewModel::fixture();
    vm.life_profile = crate::tui::life::PetLifeProfile {
        activity_level: 1.5,
        source_accent: Some(crate::tui::life::SourceAccent::Codex),
        ..Default::default()
    };

    let block = render_pet_block(&vm);

    assert_eq!(block.char_len, block.attr.string().chars().count());
}
```

- [ ] **Step 2: Run test to verify it fails or passes for current behavior**

Run: `cargo test --lib menubar::render::tests::menubar_profile_accent_is_poll_bound_and_bmp_safe`

Expected: On macOS target, FAIL until helper styling uses the profile. On non-macOS, this module may be cfg-skipped; record that in the task notes and rely on macOS verification.

- [ ] **Step 3: Add profile-aware role color**

In `src/menubar/render.rs`, add:

```rust
fn role_color_for_profile(role: PaletteRoleName, vm: &WatchViewModel) -> Rgb {
    let base = role_color(role);
    if role != PaletteRoleName::Particle && role != PaletteRoleName::Accent {
        return base;
    }
    match vm.life_profile.source_accent {
        Some(crate::tui::life::SourceAccent::Codex) => Rgb(0x86, 0xd9, 0xef),
        Some(crate::tui::life::SourceAccent::Claude) => Rgb(0xb3, 0x9d, 0xff),
        Some(crate::tui::life::SourceAccent::Balanced) => Rgb(0xf0, 0xc4, 0x6a),
        None => base,
    }
}
```

In `append_pet`, use `role_color_for_profile(segment.role, vm)` instead of `role_color(segment.role)`.

- [ ] **Step 4: Ensure menubar app stores stamped profile**

In `src/menubar/app.rs`, after receiving `PollResult`, stamp `vm.life_profile` from the local `LifeSignalState` before comparing/writing pet blocks. Do not add per-tick profile-only animation.

- [ ] **Step 5: Verify menubar rendering**

Run on macOS: `cargo test --lib menubar::render`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/menubar/render.rs src/menubar/app.rs
git commit -m "feat(menubar): add poll-bound live profile accents"
```

---

### Task 11: Final Preview, Snapshot, and Full Verification

**Files:**
- Modify: `tests/snapshots/dev_preview__watch_wide_normal_frame.snap` if intentional rendered frame changes occur.

- [ ] **Step 1: Run focused test suites**

Run:

```bash
cargo test --lib tui::life
cargo test --lib tui::panels::pet
cargo test --lib pet::speech
cargo test --lib pet::activity
cargo test --test usage_provider
cargo test --test runtime_integration
cargo test --test dev_preview
```

Expected: all PASS.

- [ ] **Step 2: Regenerate Preview Lab output for visual review**

Run:

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

Expected: command exits 0 and writes `target/glorp-preview/index.html`, `manifest.json`, text frames, cells JSON, and layout JSON.

- [ ] **Step 3: Open Preview Lab**

Run:

```bash
open target/glorp-preview/index.html
```

Expected: browser opens the review bundle. Inspect these frames:

- `watch-liveliness-s6-idle-dawn`
- `watch-liveliness-s6-hot-midday`
- `watch-liveliness-compact-s6-hot`
- `watch-liveliness-flat-s6-hot`
- `watch-liveliness-calm-mode-s6-hot`

Acceptance:

- hot differs from idle in pet/habitat cells, not only feed text
- compact has no overlapping UI and no new rows
- Flat suppresses extra ambient/weather effects
- calm-mode reduces burst/glyph intensity
- pet still reads as the same Glorp species/stage

- [ ] **Step 4: Update snapshots only after visual review**

If `cargo test --test dev_preview` reports an intentional snapshot diff, run:

```bash
INSTA_UPDATE=always cargo test --test dev_preview
```

Expected: tests pass and only intended `.snap` files change.

- [ ] **Step 5: Run formatting and lint**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: both PASS.

- [ ] **Step 6: Commit final verification artifacts**

Run:

```bash
git status --short
```

Do not stage `target/glorp-preview`; it is a generated review artifact and should remain untracked/ignored. Stage only source/test/snapshot files that `git status --short` shows as intentional:

```bash
git add <intentional-source-and-snapshot-files>
git commit -m "test(preview): verify live scene liveliness"
```

---

## Self-Review Checklist

- The signal contract is implemented before renderer consumers.
- Missing token/source detail degrades weather/accent only and does not suppress live burst.
- Freshness classification does not infer backfill from bucket count or old bucket timestamps.
- `render_pet` remains unchanged by live profile behavior.
- Compact S6 hot is explicitly tested.
- Flat and calm-mode fixtures are explicit.
- Menubar behavior is poll-bound and BMP-safe.
- Preview Lab proves cell/style differences inside pet/habitat targets.
- Branch 3 panel/day aggregate work is untouched.
