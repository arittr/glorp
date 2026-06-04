# Liveliness — Branch 1 (Correctness Fixes) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the safe, verified correctness slice of the liveliness work — make the XP progress bar stage-relative, fix the `recent_tokens_per_min` flicker, drop the menubar's next-stage spoiler, and correct stale docs/tests — independently of the contested live-activity signal.

**Architecture:** Pure display/correctness fixes in `src/commands/watch.rs` (the view-model builder), `src/tui/view_model.rs` (the `WatchViewModel`/`ProgressView` shape + fixture), `src/pet/speech.rs`, and `src/menubar/render.rs`, plus the one golden `insta` snapshot they change. No new gameplay signal, no animation work — that is Branch 2.

**Tech Stack:** Rust, ratatui TUI, `insta` (yaml snapshots, asserted from `tests/dev_preview.rs`), the `time` crate, `cargo test`/`clippy`/`fmt`.

**Branch:** Work on `feat/liveliness` (the design spec already lives there). This is the first of three sequential branches from the spec `docs/superpowers/specs/2026-06-04-glorp-liveliness-design.md`.

**Commit convention:** Conventional commits (`fix:` / `chore:` / `docs:`). End **every** commit message with the trailer:
`Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`

---

## Background the executor needs

- **The progress bug:** `build_watch_view_model_at` (`src/commands/watch.rs`, the `progress:` block ~177-215) sets `xp_in_stage = state.xp` and `xp_to_next = next_stage_xp_target(stage)` — both *absolute* — so `fraction = state.xp / next_threshold`. A pet entering S4 (xp 4.0, next 14.0) already shows ~29% and crawls. `ProgressView`'s doc comments (`src/tui/view_model.rs:126-129`) already promise stage-relative values; only the population is wrong.
- **Stage thresholds** (cumulative XP-days): `[S0 0.0, S1 0.04, S2 0.25, S3 1.0, S4 4.0, S5 14.0, S6 60.0]`. `next_stage_xp_target` already encodes the upper bounds (`src/commands/watch.rs:383-392`); we add a sibling `stage_start_xp` for the lower bounds.
- **The `recent_tokens_per_min` flicker:** it filters by `observed_at` over 60s (`src/commands/watch.rs:250-257`). Every smeared ledger row from one poll shares `observed_at = now`, so right after a poll it sums a whole delta then snaps to 0. Fix: sum by `bucket_at` (activity time) over a ~20-minute window. Its only consumer is the munch speech bubble (`src/pet/speech.rs`).
- **`rate_per_hour`** is a plain 1-hour `bucket_at` sum (`src/commands/watch.rs:178-183`) but its doc comment (`src/tui/view_model.rs:130`) lies that it is a "6h-half-life EMA", and a test is named `ema_rate_grows_with_more_recent_events` (`tests/watch_integration.rs:391`). Correct both. **Do not** touch the `xp_in_stage` doc comment — it already states the correct post-fix contract.
- **The menubar** (`src/menubar/render.rs:152-161`) renders `{pct}%  →  {next_stage_label}` from `vm.progress.fraction`. Fixing the fraction auto-corrects its percent; we additionally drop the `→ next_stage_label` spoiler to match the TUI's deliberate "next stage is a surprise."
- **Vestigial fields:** `vm.xp_current` / `vm.xp_target` (`src/commands/watch.rs:147-148`) are absolute and read by no panel or menubar (verified) — only set + carried in the fixture. After this branch they'd be a second, inconsistent XP representation. Delete them.
- **The golden snapshot** `tests/snapshots/dev_preview__watch_wide_normal_frame.snap` bakes the buggy `xp ███████░░░░░ 61` (line 30). It must be regenerated after the fraction fix. Snapshots are `insta`; regenerate with `INSTA_UPDATE=always cargo test --test dev_preview`.

## File structure

- **Modify** `src/commands/watch.rs` — add `stage_start_xp`; fix the `progress:` block; add `RECENT_ACTIVITY_WINDOW` + rename/rebase `recent_tokens_per_min` → `recent_activity_tokens`; update its call site; remove the `xp_current`/`xp_target` population; add/adjust unit tests.
- **Modify** `src/tui/view_model.rs` — remove `xp_current`/`xp_target` fields; fix the fixture's `progress` values; correct the `rate_per_hour` doc comment.
- **Modify** `src/pet/speech.rs` — rename the `recent_tokens_per_min` parameter and `MUNCH_SPEECH_THRESHOLD_PER_MIN` constant for honesty.
- **Modify** `src/menubar/render.rs` — drop the next-stage reveal.
- **Modify** `tests/watch_integration.rs` — rename the misnamed `ema_rate_*` test and its comments.
- **Regenerate** `tests/snapshots/dev_preview__watch_wide_normal_frame.snap`.

---

### Task 1: Make the progress fraction stage-relative

**Files:**
- Modify: `src/commands/watch.rs` (progress block ~177-215; add `stage_start_xp` near `:383`; update the unit test ~721-751)

- [ ] **Step 1: Update the existing unit test to expect stage-relative values (this is the failing test)**

In `src/commands/watch.rs`, replace the body of `build_watch_view_model_populates_progress_view` (currently ~735-744) so it asserts the corrected contract. Change the `state.xp` comment and the fraction assertion, and add the magnitude assertions:

```rust
        state.stage = Stage::S4;
        state.xp = 8.5; // S4 spans 4.0..14.0; 8.5 is 4.5/10.0 = 45% through the stage

        let vm = build_watch_view_model_at(&state, &db_path, now, time::UtcOffset::UTC).unwrap();
        assert_eq!(vm.progress.stage_label, "fuzz");
        assert_eq!(vm.progress.next_stage_label, "archfuzz");
        assert!(
            (vm.progress.fraction - 0.45).abs() < 0.01,
            "expected stage-relative fraction ~0.45, got {}",
            vm.progress.fraction
        );
        assert!((vm.progress.xp_in_stage - 4.5).abs() < 1e-6);
        assert!((vm.progress.xp_to_next - 10.0).abs() < 1e-6);
        assert!(
            vm.progress.rate_per_hour > 0.0,
            "expected positive rate, got {}",
            vm.progress.rate_per_hour
        );
        assert!(!vm.progress.is_max_stage);
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib build_watch_view_model_populates_progress_view`
Expected: FAIL — current code yields `fraction ≈ 0.607`, `xp_in_stage 8.5`, `xp_to_next 14.0`.

- [ ] **Step 3: Add the `stage_start_xp` helper**

In `src/commands/watch.rs`, directly below `next_stage_xp_target` (ends ~`:392`), add:

```rust
fn stage_start_xp(stage: Stage) -> f64 {
    match stage {
        Stage::S0 => 0.0,
        Stage::S1 => 0.04,
        Stage::S2 => 0.25,
        Stage::S3 => 1.0,
        Stage::S4 => 4.0,
        Stage::S5 => 14.0,
        Stage::S6 => 60.0,
    }
}
```

- [ ] **Step 4: Fix the progress block to compute stage-relative values**

In `build_watch_view_model_at`, replace these three lines in the `progress:` block (currently ~184-186):

```rust
            let is_max = matches!(stage, Stage::S6);
            let xp_to_next = next_stage_xp_target(stage);
            let xp_in_stage = state.xp;
```

with:

```rust
            let is_max = matches!(stage, Stage::S6);
            let stage_start = stage_start_xp(stage);
            let xp_in_stage = state.xp - stage_start;
            let xp_to_next = next_stage_xp_target(stage) - stage_start;
```

Leave the `fraction` computation below it unchanged — `if xp_to_next <= 0.0 || is_max { 1.0 }` already guards S6 (where `xp_to_next` becomes 0.0).

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test --lib build_watch_view_model_populates_progress_view`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/commands/watch.rs
git commit  # fix(watch): make xp progress bar stage-relative, not absolute
```

---

### Task 2: Fix the `recent_tokens_per_min` flicker (rebase to bucket_at)

**Files:**
- Modify: `src/commands/watch.rs` (the fn ~249-257, its call site ~168-172, a new const, a new unit test)
- Modify: `src/pet/speech.rs` (parameter + constant rename)

- [ ] **Step 1: Write the failing test**

In `src/commands/watch.rs`'s `#[cfg(test)] mod tests`, add (the module already imports `NormalizedUsageEvent`, `OffsetDateTime`, `Duration` via the existing rate/bucket tests):

```rust
    #[test]
    fn recent_activity_tokens_uses_bucket_at_not_observed_at() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        // Build an event observed `now` but whose activity time is `bucket_offset` ago.
        let event = |bucket_offset: Duration, tokens: f64| NormalizedUsageEvent {
            observed_at: now,
            bucket_at: now - bucket_offset,
            effective_tokens: tokens,
            ..NormalizedUsageEvent::for_test_at(now, tokens)
        };
        // Catchup: a huge delta whose activity happened 3h ago — must NOT count.
        assert_eq!(
            recent_activity_tokens(&[event(Duration::hours(3), 1_000_000.0)], now),
            0.0
        );
        // Same catchup plus fresh in-window activity — only the fresh tokens count.
        assert_eq!(
            recent_activity_tokens(
                &[
                    event(Duration::hours(3), 1_000_000.0),
                    event(Duration::minutes(5), 12_000.0),
                ],
                now
            ),
            12_000.0
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib recent_activity_tokens_uses_bucket_at_not_observed_at`
Expected: FAIL — `recent_activity_tokens` does not exist yet (and the old `recent_tokens_per_min` filters by `observed_at`, which would count the catchup).

- [ ] **Step 3: Replace the function (rename + rebase to bucket_at, 20-minute window)**

In `src/commands/watch.rs`, replace the function (currently ~249-257):

```rust
/// Tokens observed in the last 60 seconds, returned as a per-minute rate.
fn recent_tokens_per_min(usage_events: &[NormalizedUsageEvent], now: OffsetDateTime) -> f64 {
    let cutoff = now - Duration::minutes(1);
    usage_events
        .iter()
        .filter(|e| e.observed_at >= cutoff)
        .map(|e| e.effective_tokens)
        .sum()
}
```

with:

```rust
/// Window over which `recent_activity_tokens` sums recent activity. Spans at
/// least two 10-minute smear buckets so the value decays smoothly instead of
/// stepping.
const RECENT_ACTIVITY_WINDOW: Duration = Duration::minutes(20);

/// Effective tokens whose activity time (`bucket_at`) falls in the last
/// `RECENT_ACTIVITY_WINDOW`. Uses `bucket_at`, not `observed_at`: a catchup
/// poll back-dates a fat delta across past buckets while stamping every
/// smeared row with `observed_at = now`, so an `observed_at` window spikes to
/// the whole delta on one poll and snaps to zero on the next. Summing by
/// `bucket_at` reflects when activity actually happened.
fn recent_activity_tokens(usage_events: &[NormalizedUsageEvent], now: OffsetDateTime) -> f64 {
    let cutoff = now - RECENT_ACTIVITY_WINDOW;
    usage_events
        .iter()
        .filter(|e| e.bucket_at >= cutoff)
        .map(|e| e.effective_tokens)
        .sum()
}
```

- [ ] **Step 4: Update the call site**

In `build_watch_view_model_at` (~168-172), change the `current_speech` argument:

```rust
        current_speech: crate::pet::speech::current_pet_speech(
            mood,
            recent_activity_tokens(&recent_usage, now),
            now,
        ),
```

- [ ] **Step 5: Rename the speech parameter and constant for honesty**

In `src/pet/speech.rs`, rename the constant (the value passed is now "tokens in the recent activity window", not a per-minute rate):

```rust
/// Effective tokens in the recent activity window above which speech defaults
/// to feeding reactions ("yum!" etc.), regardless of mood. Branch 2 will
/// re-point this onto the normalized live-activity signal.
const MUNCH_SPEECH_THRESHOLD: f64 = 30_000.0;
```

and update `current_pet_speech`'s parameter name + the comparison (currently ~26-40):

```rust
pub fn current_pet_speech(
    mood: Mood,
    recent_activity_tokens: f64,
    now: OffsetDateTime,
) -> Option<String> {
    let cycle_pos = now.unix_timestamp().rem_euclid(SPEECH_CYCLE_SECS);
    if cycle_pos >= SPEECH_VISIBLE_SECS {
        return None;
    }

    if recent_activity_tokens >= MUNCH_SPEECH_THRESHOLD {
        return Some(pick_munch_phrase(now));
    }

    Some(mood_phrase(mood, now))
}
```

(The `speech.rs` test `munch_speech_fires_on_high_token_rate` passes `50_000.0`, which still exceeds the threshold — it keeps passing unchanged.)

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test --lib recent_activity_tokens_uses_bucket_at_not_observed_at && cargo test --lib speech`
Expected: PASS (new test + the speech module tests).

- [ ] **Step 7: Commit**

```bash
git add src/commands/watch.rs src/pet/speech.rs
git commit  # fix(watch): base recent-activity speech on bucket_at, not observed_at
```

---

### Task 3: Fix the `WatchViewModel` fixture's progress values

**Files:**
- Modify: `src/tui/view_model.rs` (the `fixture()` `progress` block ~235-243)

- [ ] **Step 1: Update the fixture to the stage-relative contract**

In `src/tui/view_model.rs`, in `fixture()`, change the `ProgressView` so its values are internally consistent (S4 pet, xp 8.5, span 4.0..14.0 → 4.5/10.0 = 0.45):

```rust
            progress: ProgressView {
                stage_label: "fuzz".to_string(),
                next_stage_label: "archfuzz".to_string(),
                fraction: 0.45,
                xp_in_stage: 4.5,
                xp_to_next: 10.0,
                rate_per_hour: 109_000.0,
                is_max_stage: false,
            },
```

- [ ] **Step 2: Run the panels + view-model tests to verify nothing regressed**

Run: `cargo test --lib progress && cargo test --lib view_model`
Expected: PASS (these tests assert presence of the bar and absence of the next-stage label, not the numeric value).

- [ ] **Step 3: Commit**

```bash
git add src/tui/view_model.rs
git commit  # fix(view-model): make fixture progress values stage-relative
```

---

### Task 4: Drop the menubar's next-stage spoiler

**Files:**
- Modify: `src/menubar/render.rs` (the `xp` stat row ~155-160)

- [ ] **Step 1: Remove the `→ next_stage_label` reveal**

In `src/menubar/render.rs`, in `append_stats`, replace the `else` branch of the `is_max_stage` check (currently ~154-160):

```rust
    } else {
        let pct = ((vm.progress.fraction * 100.0).round() as i32).clamp(0, 100);
        push_stat_row(runs, "xp", format!("{}%", pct));
    }
```

(The percent now reads the corrected stage-relative `fraction` automatically. The next stage stays a surprise, matching the TUI.)

- [ ] **Step 2: Verify the crate builds and menubar tests pass**

Run: `cargo build && cargo test --lib menubar`
Expected: PASS — no menubar test asserts the arrow/next-stage text.

- [ ] **Step 3: Commit**

```bash
git add src/menubar/render.rs
git commit  # fix(menubar): stop revealing the next stage; keep it a surprise
```

---

### Task 5: Correct the stale `rate_per_hour` doc comment and test name

**Files:**
- Modify: `src/tui/view_model.rs` (doc comment ~130)
- Modify: `tests/watch_integration.rs` (test `ema_rate_grows_with_more_recent_events` ~390-431)

- [ ] **Step 1: Fix the doc comment**

In `src/tui/view_model.rs`, change the `rate_per_hour` field doc comment (~130) from:

```rust
    /// 6h-half-life EMA, effective tokens / hour.
    pub rate_per_hour: f64,
```

to:

```rust
    /// Effective tokens observed in the last hour (sum over `bucket_at`).
    pub rate_per_hour: f64,
```

- [ ] **Step 2: Rename the misnamed test and fix its comments**

In `tests/watch_integration.rs`, rename the test and correct the two "EMA" references (~390-431):

```rust
#[test]
fn rate_per_hour_grows_with_more_recent_events() {
```

Change the inline comment `// Add one large event right at now — it carries maximum EMA weight.` to:

```rust
    // Add one large event right at now — it lands inside the 1-hour window.
```

and the assertion message `"ema must grow with more recent contribution ..."` to:

```rust
        "rate must grow with more recent contribution (a={rate_a}, b={rate_b})"
```

- [ ] **Step 3: Verify the renamed test passes**

Run: `cargo test --test watch_integration rate_per_hour_grows_with_more_recent_events`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/tui/view_model.rs tests/watch_integration.rs
git commit  # docs: correct the rate_per_hour 'EMA' lie in comment and test name
```

---

### Task 6: Delete the vestigial absolute `xp_current` / `xp_target`

**Files:**
- Modify: `src/tui/view_model.rs` (struct fields + fixture)
- Modify: `src/commands/watch.rs` (population in `build_watch_view_model_at` ~147-148)

- [ ] **Step 1: Remove the struct fields**

In `src/tui/view_model.rs`, delete these two fields from `WatchViewModel` (currently ~19-20):

```rust
    pub xp_current: f64,
    pub xp_target: f64,
```

- [ ] **Step 2: Remove them from the fixture**

In `fixture()`, delete the two lines (currently ~175-176):

```rust
            xp_current: 42_000.0,
            xp_target: 100_000.0,
```

- [ ] **Step 3: Remove the population in the view-model builder**

In `src/commands/watch.rs`, in the `WatchViewModel { ... }` literal in `build_watch_view_model_at`, delete the two lines (currently ~147-148):

```rust
        xp_current: state.xp,
        xp_target: next_stage_xp_target(stage),
```

- [ ] **Step 4: Verify the whole crate compiles (the compiler proves there were no other readers)**

Run: `cargo build --all-targets`
Expected: PASS with no errors. If anything fails to compile, a reader exists — stop and reassess rather than re-adding the fields blindly.

- [ ] **Step 5: Commit**

```bash
git add src/tui/view_model.rs src/commands/watch.rs
git commit  # chore(view-model): drop vestigial absolute xp_current/xp_target
```

---

### Task 7: Regenerate the golden snapshot and verify the full suite

**Files:**
- Regenerate: `tests/snapshots/dev_preview__watch_wide_normal_frame.snap`

- [ ] **Step 1: Confirm the snapshot is currently stale**

Run: `cargo test --test dev_preview`
Expected: FAIL — the `watch_wide_normal_frame` snapshot still shows `xp ███████░░░░░ 61`, but the corrected fraction renders a lower percent and different bar glyphs.

- [ ] **Step 2: Regenerate the snapshot in place**

Run: `INSTA_UPDATE=always cargo test --test dev_preview`
Expected: PASS (insta rewrites the `.snap`).

- [ ] **Step 3: Inspect the diff and confirm it is limited to expected changes**

Run: `git diff tests/snapshots/dev_preview__watch_wide_normal_frame.snap`
Expected: the `xp` line's percent drops from `61` to its corrected stage-relative value and the bar-fill glyphs shift accordingly. If the speech bubble or feed lines also changed, confirm the change is consistent with the `recent_activity_tokens` rebase (Task 2) and nothing else. No other panels should change.

- [ ] **Step 4: Run the full suite, clippy, and fmt — all must be green**

Run:
```bash
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```
Expected: all PASS. (`cargo fmt --check` should report no diffs; if it does, run `cargo fmt` and include the result in this commit.)

- [ ] **Step 5: Commit**

```bash
git add tests/snapshots/dev_preview__watch_wide_normal_frame.snap
git commit  # test: regenerate watch snapshot for stage-relative xp bar
```

---

## Self-review against the spec (Branch 1 scope)

- **Stage-relative fraction** → Task 1 (helper + computation + test). ✔
- **All fraction consumers** → snapshot (Task 7), fixture (Task 3), watch.rs test (Task 1), menubar percent (auto via Task 1; spoiler dropped Task 4). ✔
- **`recent_tokens_per_min` flicker fix** → Task 2 (bucket_at, 20-min window, honest rename; munch threshold reachable). ✔
- **Doc/test honesty** (`rate_per_hour` comment + `ema_rate_*` test; `xp_in_stage` comment left intact) → Task 5. ✔
- **Vestigial `xp_current`/`xp_target`** → Task 6. ✔
- **Out of scope here (Branch 2/3):** the magnitude readout, pips, "always show pace", the activity signal, animation, `activity.rs` munch/idle, the day-axis/best-day/sparkline work. None appear in this plan. ✔

**Type/name consistency:** `stage_start_xp` and `recent_activity_tokens` are used with the same signatures everywhere they appear; `MUNCH_SPEECH_THRESHOLD` replaces `MUNCH_SPEECH_THRESHOLD_PER_MIN` at its single definition and use; `xp_current`/`xp_target` are removed from all three sites together (compiler-verified in Task 6).
