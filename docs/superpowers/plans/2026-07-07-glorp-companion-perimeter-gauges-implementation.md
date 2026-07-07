# Glorp Companion Perimeter Gauges Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the approved B2 macOS companion perimeter gauge: outer XP, middle today-vs-yesterday, inner 10-minute pace, plus the neutral three-line HUD text stack.

**Architecture:** Keep exact telemetry in `WatchViewModel` and the native companion HUD. Put reusable geometry, fill normalization, color, and text helpers in cfg-free `src/round/hud.rs`; `src/companion/app.rs` becomes a thin AppKit painter. Extend Preview Lab with a renderer-neutral HUD JSON artifact so visual review can inspect the native companion HUD contract without moving exact counts into `RoundSceneModel`.

**Tech Stack:** Rust, `time`, `ratatui` Preview Lab artifacts, SQLite-backed `UsageStore`, macOS `objc2-app-kit` AppKit drawing.

## Global Constraints

- Do not create a branch unless Drew asks for one.
- Do not touch unrelated dirty files: `src/storage/usage_store.rs` and `tests/storage_privacy.rs`.
- Use provider-day Tokenmaxxing snapshot totals for the daily comparison.
- `PACE_SOFT_CAP_10M_TOKENS = 50_000_000`.
- The gauge gap is `70.0` degrees, matching the current companion ring.
- Lane order is `xp` outside, `daily` middle, `pace` inside.
- Lane colors are violet for XP, cyan for daily, amber for pace.
- Lane caps are round for track and fill paths.
- The daily lane fill clamps to `0.0..=1.0`; daily text may show values greater than `100%`.
- The companion text stack is exactly `{today_total}`, `{daily_percent}`, `{pace}/10m`; it must not include `/hr`.
- No source labels, project names, file paths, prompts, responses, costs, quotas, ETAs, streaks, or productivity scoring on the companion HUD.
- Exact telemetry stays out of `RoundSceneModel`.
- Preview Lab manifest schema is currently `5`; bump it to `6` when adding `files.hud` and artifact type `hud`.
- Add no new dependencies.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `src/tui/view_model.rs` | Add `DailyComparison`, its snapshot-derived constructor, and `WatchViewModel.daily_comparison`. |
| `src/commands/watch.rs` | Query yesterday's provider-day snapshot and populate `DailyComparison` beside existing today snapshot fields. |
| `tests/watch_integration.rs` | Prove daily comparison uses real provider-day snapshots and degrades missing, stale, zero, and over-100 cases deterministically. |
| `src/round/hud.rs` | Add perimeter lane geometry, lane colors, line-cap metadata, pace normalization, daily percent formatting, and companion HUD text helpers. |
| `src/companion/app.rs` | Replace the single thin growth ring with the three-lane gauge and replace the two-line rate stack with `% yday` plus `/10m`. |
| `src/dev_preview/contract.rs` | Add `PreviewHudArtifact` and serialize lane geometry, colors, fills, caps, and HUD text. |
| `src/dev_preview/export.rs` | Add `files.hud`, artifact type `hud`, review links, and manifest schema `6`. |
| `src/dev_preview/scenarios.rs` | Write `frames/<id>.hud.json`, list HUD artifacts, and add manifest file paths. |
| `src/dev_preview/round.rs` | Attach HUD artifacts to round preview frames and add daily/pace fixture variants. |
| `tests/dev_preview.rs` | Assert HUD artifacts, scenario coverage, schema version `6`, and scene-artifact privacy. |
| `tests/round_scene.rs` | Assert exact companion HUD metrics do not enter `RoundSceneModel`. |

## Task 1: DailyComparison View Model

**Files:**
- Modify: `src/tui/view_model.rs`
- Modify: `src/commands/watch.rs`
- Test: `tests/watch_integration.rs`

**Interfaces:**
- Consumes: `UsageStore::snapshot_totals_for_provider_day(day: time::Date) -> Result<SnapshotResult<DayTotals>>`.
- Produces: `WatchViewModel.daily_comparison: DailyComparison`.
- Produces: `DailyComparison::from_snapshots(today: &SnapshotResult<DayTotals>, yesterday: &SnapshotResult<DayTotals>) -> DailyComparison`.

- [ ] **Step 1: Write failing watch integration tests**

Add these tests near the existing snapshot-history tests in `tests/watch_integration.rs`:

```rust
#[test]
fn daily_comparison_uses_current_tokenmaxxing_provider_day_snapshots() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage = UsageStore::open(&usage_db).unwrap();
    let now = datetime!(2026 - 07 - 06 20:00 UTC);
    let today = time::macros::date!(2026 - 07 - 06);
    let yesterday = time::macros::date!(2026 - 07 - 05);

    seed_snapshot_for_test(&mut usage, today, "claude-code", 125_000.0, now);
    seed_snapshot_for_test(
        &mut usage,
        yesterday,
        "claude-code",
        100_000.0,
        now - Duration::days(1),
    );

    let vm = build_watch_view_model_for_test_at(&mech_state(), &usage_db, now).unwrap();

    assert_eq!(vm.daily_comparison.today_provider_day, today);
    assert_eq!(vm.daily_comparison.yesterday_provider_day, yesterday);
    assert_eq!(vm.daily_comparison.today_tokens, 125_000.0);
    assert_eq!(vm.daily_comparison.yesterday_tokens, Some(100_000.0));
    assert_eq!(
        vm.daily_comparison.today_snapshot_state,
        glorp::usage::snapshot::SnapshotState::Current
    );
    assert_eq!(
        vm.daily_comparison.yesterday_snapshot_state,
        glorp::usage::snapshot::SnapshotState::Current
    );
    assert_eq!(vm.daily_comparison.unavailable_reason, None);
    assert!(
        (vm.daily_comparison.fraction_of_yesterday.unwrap() - 1.25).abs() < 1e-9
    );
}

#[test]
fn daily_comparison_degrades_missing_and_zero_yesterday_without_fill() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage = UsageStore::open(&usage_db).unwrap();
    let now = datetime!(2026 - 07 - 06 20:00 UTC);
    let today = time::macros::date!(2026 - 07 - 06);
    let yesterday = time::macros::date!(2026 - 07 - 05);

    seed_snapshot_for_test(&mut usage, today, "claude-code", 42_000.0, now);
    let missing = build_watch_view_model_for_test_at(&mech_state(), &usage_db, now).unwrap();

    assert_eq!(missing.daily_comparison.yesterday_provider_day, yesterday);
    assert_eq!(missing.daily_comparison.yesterday_tokens, None);
    assert_eq!(
        missing.daily_comparison.yesterday_snapshot_state,
        glorp::usage::snapshot::SnapshotState::Missing
    );
    assert_eq!(
        missing.daily_comparison.unavailable_reason.as_deref(),
        Some("yesterday-missing")
    );
    assert_eq!(missing.daily_comparison.fraction_of_yesterday, None);

    seed_snapshot_for_test(&mut usage, yesterday, "claude-code", 0.0, now - Duration::days(1));
    let zero = build_watch_view_model_for_test_at(&mech_state(), &usage_db, now).unwrap();

    assert_eq!(zero.daily_comparison.yesterday_tokens, Some(0.0));
    assert_eq!(
        zero.daily_comparison.unavailable_reason.as_deref(),
        Some("yesterday-zero")
    );
    assert_eq!(zero.daily_comparison.fraction_of_yesterday, None);
}

#[test]
fn daily_comparison_rejects_stale_yesterday_snapshot() {
    let dir = tempdir().unwrap();
    let usage_db = dir.path().join("usage.sqlite");
    let mut usage = UsageStore::open(&usage_db).unwrap();
    let now = datetime!(2026 - 07 - 06 20:00 UTC);
    let today = time::macros::date!(2026 - 07 - 06);
    let yesterday = time::macros::date!(2026 - 07 - 05);

    seed_snapshot_for_test(&mut usage, today, "claude-code", 42_000.0, now);
    seed_snapshot_for_test(
        &mut usage,
        yesterday,
        "claude-code",
        21_000.0,
        now - Duration::days(1),
    );
    seed_blocked_snapshot_failure_for_test(&mut usage, yesterday, now);

    let vm = build_watch_view_model_for_test_at(&mech_state(), &usage_db, now).unwrap();

    assert_eq!(
        vm.daily_comparison.yesterday_snapshot_state,
        glorp::usage::snapshot::SnapshotState::Stale
    );
    assert_eq!(vm.daily_comparison.yesterday_tokens, Some(21_000.0));
    assert_eq!(
        vm.daily_comparison.unavailable_reason.as_deref(),
        Some("yesterday-stale")
    );
    assert_eq!(vm.daily_comparison.fraction_of_yesterday, None);
}

fn seed_blocked_snapshot_failure_for_test(
    usage: &mut UsageStore,
    day: time::Date,
    observed_at: OffsetDateTime,
) {
    usage
        .record_snapshot_failure(&glorp::usage::snapshot::ProviderSnapshotDiagnosticInput {
            diagnostic_kind: "run_blocked".into(),
            collector_scope_id: "claude-code:local-usage".into(),
            replacement_scope_id: Some("claude-code:local-usage".into()),
            requested_provider_days: vec![day],
            provider_day: Some(day),
            reason_code: "helper_exit".into(),
            message: "ccusage helper exited while refreshing snapshot".into(),
            observed_at,
        })
        .unwrap();
}
```

- [ ] **Step 2: Run the new tests and confirm they fail on missing fields**

Run:

```bash
cargo test --test watch_integration daily_comparison_ -- --nocapture
```

Expected: FAIL with errors mentioning `daily_comparison` or `DailyComparison`.

- [ ] **Step 3: Add the view-model type and constructor**

Add this type below `WatchViewModel` in `src/tui/view_model.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct DailyComparison {
    pub today_provider_day: time::Date,
    pub yesterday_provider_day: time::Date,
    pub today_tokens: f64,
    pub yesterday_tokens: Option<f64>,
    pub today_snapshot_state: crate::usage::snapshot::SnapshotState,
    pub yesterday_snapshot_state: crate::usage::snapshot::SnapshotState,
    pub today_observed_at: Option<time::OffsetDateTime>,
    pub yesterday_observed_at: Option<time::OffsetDateTime>,
    pub unavailable_reason: Option<String>,
    pub fraction_of_yesterday: Option<f64>,
}

impl DailyComparison {
    pub fn from_snapshots(
        today: &crate::usage::snapshot::SnapshotResult<crate::usage::snapshot::DayTotals>,
        yesterday: &crate::usage::snapshot::SnapshotResult<crate::usage::snapshot::DayTotals>,
    ) -> Self {
        let today_tokens = snapshot_total_tokens(today).unwrap_or(0.0);
        let yesterday_tokens = snapshot_total_tokens(yesterday);
        let unavailable_reason = daily_unavailable_reason(today, yesterday);
        let fraction_of_yesterday = unavailable_reason.as_ref().is_none().then(|| {
            today_tokens / yesterday_tokens.expect("validated yesterday total")
        });

        Self {
            today_provider_day: today.provider_day,
            yesterday_provider_day: yesterday.provider_day,
            today_tokens,
            yesterday_tokens,
            today_snapshot_state: today.state,
            yesterday_snapshot_state: yesterday.state,
            today_observed_at: today.observed_at,
            yesterday_observed_at: yesterday.observed_at,
            unavailable_reason,
            fraction_of_yesterday,
        }
    }
}

fn snapshot_total_tokens(
    result: &crate::usage::snapshot::SnapshotResult<crate::usage::snapshot::DayTotals>,
) -> Option<f64> {
    result.value.as_ref().map(|totals| totals.total_tokens)
}

fn daily_unavailable_reason(
    today: &crate::usage::snapshot::SnapshotResult<crate::usage::snapshot::DayTotals>,
    yesterday: &crate::usage::snapshot::SnapshotResult<crate::usage::snapshot::DayTotals>,
) -> Option<String> {
    use crate::usage::snapshot::SnapshotState;

    if today.state != SnapshotState::Current {
        return Some(format!("today-{}", snapshot_state_slug(today.state)));
    }
    if yesterday.state != SnapshotState::Current {
        return Some(format!("yesterday-{}", snapshot_state_slug(yesterday.state)));
    }

    let Some(today_tokens) = snapshot_total_tokens(today) else {
        return Some("today-missing".to_string());
    };
    let Some(yesterday_tokens) = snapshot_total_tokens(yesterday) else {
        return Some("yesterday-missing".to_string());
    };

    if !today_tokens.is_finite()
        || !yesterday_tokens.is_finite()
        || today_tokens < 0.0
        || yesterday_tokens < 0.0
    {
        return Some("non-finite-total".to_string());
    }
    if yesterday_tokens == 0.0 {
        return Some("yesterday-zero".to_string());
    }

    None
}

fn snapshot_state_slug(state: crate::usage::snapshot::SnapshotState) -> &'static str {
    match state {
        crate::usage::snapshot::SnapshotState::Current => "current",
        crate::usage::snapshot::SnapshotState::Stale => "stale",
        crate::usage::snapshot::SnapshotState::Missing => "missing",
        crate::usage::snapshot::SnapshotState::Blocked => "blocked",
    }
}
```

Add the field to `WatchViewModel` near the existing snapshot fields:

```rust
    pub today_snapshot_reason: Option<String>,
    pub daily_comparison: DailyComparison,
    pub recent_daily_effective_tokens: Vec<f64>,
```

Update `WatchViewModel::fixture()`:

```rust
            today_snapshot_reason: None,
            daily_comparison: DailyComparison {
                today_provider_day: time::macros::date!(2026 - 07 - 06),
                yesterday_provider_day: time::macros::date!(2026 - 07 - 05),
                today_tokens: 18_420.0,
                yesterday_tokens: Some(16_000.0),
                today_snapshot_state: crate::usage::snapshot::SnapshotState::Current,
                yesterday_snapshot_state: crate::usage::snapshot::SnapshotState::Current,
                today_observed_at: None,
                yesterday_observed_at: None,
                unavailable_reason: None,
                fraction_of_yesterday: Some(18_420.0 / 16_000.0),
            },
            recent_daily_effective_tokens: vec![
```

- [ ] **Step 4: Populate `daily_comparison` from `UsageStore`**

In `src/commands/watch.rs`, add `DailyComparison` to the `view_model` import list:

```rust
            BioView, DailyComparison, EarnedHabitatPropView, EventView, HabitatView,
```

In `build_watch_view_model_at`, query yesterday directly after the today snapshot:

```rust
    let today_snapshot = usage_store.snapshot_totals_for_provider_day(provider_day)?;
    let yesterday_provider_day = provider_day.previous_day().unwrap_or(provider_day);
    let yesterday_snapshot = usage_store.snapshot_totals_for_provider_day(yesterday_provider_day)?;
    let daily_comparison =
        DailyComparison::from_snapshots(&today_snapshot, &yesterday_snapshot);
    let source_snapshot = usage_store.snapshot_totals_by_source_for_provider_day(provider_day)?;
```

Add the field to the `WatchViewModel` construction:

```rust
        today_snapshot_state: today_snapshot.state,
        today_snapshot_reason: today_snapshot.reason.clone(),
        daily_comparison,
        recent_daily_effective_tokens,
```

- [ ] **Step 5: Run Task 1 tests**

Run:

```bash
cargo test --test watch_integration daily_comparison_ -- --nocapture
```

Expected: PASS for the three `daily_comparison_` tests.

- [ ] **Step 6: Commit Task 1**

```bash
git add src/tui/view_model.rs src/commands/watch.rs tests/watch_integration.rs
git commit -m "feat: add companion daily comparison model"
```

## Task 2: Pure Perimeter HUD Helpers

**Files:**
- Modify: `src/round/hud.rs`

**Interfaces:**
- Produces: `COMPANION_GAUGE_GAP_DEG: f64 = 70.0`.
- Produces: `PACE_SOFT_CAP_10M_TOKENS: f64 = 50_000_000.0`.
- Produces: `perimeter_gauge_layout(cx: f64, cy: f64, aperture_radius: f64, gap_deg: f64) -> PerimeterGaugeLayout`.
- Produces: `companion_pace_fraction(current_10m_tokens: f64) -> f64`.
- Produces: `format_daily_percent(fraction_of_yesterday: Option<f64>) -> String`.
- Produces: `companion_hud_text(today_tokens: f64, daily_fraction: Option<f64>, pulse_10m_tokens: f64) -> CompanionHudText`.
- Consumes: Existing `GrowthRing`, `growth_ring_layout`, and `RoundColor`.

- [ ] **Step 1: Write failing helper tests**

Append these tests inside `#[cfg(test)] mod tests` in `src/round/hud.rs`:

```rust
    #[test]
    fn perimeter_gauge_layout_keeps_three_round_lanes_inside_aperture() {
        let layout = perimeter_gauge_layout(180.0, 180.0, 180.0, COMPANION_GAUGE_GAP_DEG);

        assert_eq!(layout.xp.cap, LineCap::Round);
        assert_eq!(layout.daily.cap, LineCap::Round);
        assert_eq!(layout.pace.cap, LineCap::Round);

        assert_eq!(layout.xp.ring.track_start_deg, layout.daily.ring.track_start_deg);
        assert_eq!(layout.daily.ring.track_start_deg, layout.pace.ring.track_start_deg);
        assert_eq!(layout.xp.ring.track_sweep_deg, layout.daily.ring.track_sweep_deg);
        assert_eq!(layout.daily.ring.track_sweep_deg, layout.pace.ring.track_sweep_deg);

        assert!(layout.xp.ring.radius > layout.daily.ring.radius);
        assert!(layout.daily.ring.radius > layout.pace.ring.radius);
        assert!(layout.xp.stroke_width > layout.daily.stroke_width);
        assert!(layout.daily.stroke_width > layout.pace.stroke_width);

        let xp_outer_edge = layout.xp.ring.radius + layout.xp.stroke_width / 2.0;
        let pace_inner_edge = layout.pace.ring.radius - layout.pace.stroke_width / 2.0;

        assert!(xp_outer_edge <= 177.0);
        assert!(pace_inner_edge > 180.0 * 0.72);
    }

    #[test]
    fn pace_fraction_uses_named_soft_cap_and_clamps_bad_inputs() {
        assert_eq!(companion_pace_fraction(0.0), 0.0);
        assert!((companion_pace_fraction(PACE_SOFT_CAP_10M_TOKENS) - 0.632).abs() < 0.002);
        assert!(
            (companion_pace_fraction(PACE_SOFT_CAP_10M_TOKENS * 2.0) - 0.865).abs() < 0.002
        );
        assert!(companion_pace_fraction(PACE_SOFT_CAP_10M_TOKENS * 100.0) <= 1.0);
        assert_eq!(companion_pace_fraction(-1.0), 0.0);
        assert_eq!(companion_pace_fraction(f64::NAN), 0.0);
        assert_eq!(companion_pace_fraction(f64::INFINITY), 0.0);
    }

    #[test]
    fn companion_hud_text_formats_total_daily_percent_and_pace_only() {
        let text = companion_hud_text(842_000_000.0, Some(1.244), 31_000_000.0);

        assert_eq!(text.today_total, "842M");
        assert_eq!(text.daily_percent, "124% yday");
        assert_eq!(text.pace, "31M/10m");
        assert!(!text.pace.contains("/hr"));
    }

    #[test]
    fn daily_percent_text_preserves_stack_when_unavailable_and_caps_extreme_values() {
        assert_eq!(format_daily_percent(None), "--% yday");
        assert_eq!(format_daily_percent(Some(0.944)), "94% yday");
        assert_eq!(format_daily_percent(Some(10.5)), "999%+ yday");
        assert_eq!(format_daily_percent(Some(f64::NAN)), "--% yday");
        assert_eq!(format_daily_percent(Some(f64::INFINITY)), "--% yday");
    }
```

- [ ] **Step 2: Run the helper tests and confirm they fail on missing symbols**

Run:

```bash
cargo test --lib round::hud -- --nocapture
```

Expected: FAIL with errors mentioning `perimeter_gauge_layout`, `LineCap`, `PACE_SOFT_CAP_10M_TOKENS`, or `companion_hud_text`.

- [ ] **Step 3: Add constants, lane structs, colors, and geometry**

Add this code in `src/round/hud.rs` after `GrowthRing`:

```rust
pub const COMPANION_GAUGE_GAP_DEG: f64 = 70.0;
pub const PACE_SOFT_CAP_10M_TOKENS: f64 = 50_000_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineCap {
    Butt,
    Round,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaugeLane {
    pub ring: GrowthRing,
    pub stroke_width: f64,
    pub cap: LineCap,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerimeterGaugeLayout {
    pub xp: GaugeLane,
    pub daily: GaugeLane,
    pub pace: GaugeLane,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaugeLaneColors {
    pub track: RoundColor,
    pub fill: RoundColor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerimeterGaugeColors {
    pub xp: GaugeLaneColors,
    pub daily: GaugeLaneColors,
    pub pace: GaugeLaneColors,
}

pub fn perimeter_gauge_layout(
    cx: f64,
    cy: f64,
    aperture_radius: f64,
    gap_deg: f64,
) -> PerimeterGaugeLayout {
    let outer_inset_px = 3.0_f64.max(aperture_radius * 0.012);
    let xp_width = (aperture_radius * 0.050).clamp(6.0, 16.0);
    let daily_width = (aperture_radius * 0.040).clamp(5.0, 13.0);
    let pace_width = (aperture_radius * 0.034).clamp(4.0, 11.0);
    let lane_gap = (aperture_radius * 0.010).clamp(1.5, 4.0);

    let xp_radius = aperture_radius - outer_inset_px - xp_width / 2.0;
    let daily_radius = xp_radius - xp_width / 2.0 - lane_gap - daily_width / 2.0;
    let pace_radius = daily_radius - daily_width / 2.0 - lane_gap - pace_width / 2.0;

    PerimeterGaugeLayout {
        xp: GaugeLane {
            ring: growth_ring_layout(cx, cy, xp_radius, gap_deg),
            stroke_width: xp_width,
            cap: LineCap::Round,
        },
        daily: GaugeLane {
            ring: growth_ring_layout(cx, cy, daily_radius, gap_deg),
            stroke_width: daily_width,
            cap: LineCap::Round,
        },
        pace: GaugeLane {
            ring: growth_ring_layout(cx, cy, pace_radius, gap_deg),
            stroke_width: pace_width,
            cap: LineCap::Round,
        },
    }
}

pub fn perimeter_gauge_colors() -> PerimeterGaugeColors {
    PerimeterGaugeColors {
        xp: GaugeLaneColors {
            track: RoundColor(0.71, 0.71, 0.78, 0.16),
            fill: RoundColor(0.61, 0.48, 0.88, 0.90),
        },
        daily: GaugeLaneColors {
            track: RoundColor(0.52, 0.80, 0.88, 0.14),
            fill: RoundColor(0.36, 0.84, 0.95, 0.82),
        },
        pace: GaugeLaneColors {
            track: RoundColor(0.96, 0.68, 0.31, 0.13),
            fill: RoundColor(0.98, 0.67, 0.27, 0.86),
        },
    }
}
```

- [ ] **Step 4: Add normalization and text helpers**

Add this code in `src/round/hud.rs` near the rate color helper:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanionHudText {
    pub today_total: String,
    pub daily_percent: String,
    pub pace: String,
}

pub fn companion_pace_fraction(current_10m_tokens: f64) -> f64 {
    if !current_10m_tokens.is_finite() || current_10m_tokens <= 0.0 {
        return 0.0;
    }
    (1.0 - (-current_10m_tokens / PACE_SOFT_CAP_10M_TOKENS).exp()).clamp(0.0, 1.0)
}

pub fn daily_fraction_for_gauge(fraction_of_yesterday: Option<f64>) -> f64 {
    fraction_of_yesterday
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.clamp(0.0, 1.0))
        .unwrap_or(0.0)
}

pub fn format_daily_percent(fraction_of_yesterday: Option<f64>) -> String {
    let Some(fraction) = fraction_of_yesterday else {
        return "--% yday".to_string();
    };
    if !fraction.is_finite() || fraction < 0.0 {
        return "--% yday".to_string();
    }

    let percent = (fraction * 100.0).round();
    if percent > 999.0 {
        "999%+ yday".to_string()
    } else {
        format!("{percent:.0}% yday")
    }
}

pub fn companion_hud_text(
    today_tokens: f64,
    daily_fraction: Option<f64>,
    pulse_10m_tokens: f64,
) -> CompanionHudText {
    CompanionHudText {
        today_total: crate::format::format_tokens(today_tokens),
        daily_percent: format_daily_percent(daily_fraction),
        pace: format!("{}/10m", crate::format::format_tokens(pulse_10m_tokens.max(0.0))),
    }
}
```

- [ ] **Step 5: Run Task 2 tests**

Run:

```bash
cargo test --lib round::hud -- --nocapture
```

Expected: PASS for all `round::hud` tests.

- [ ] **Step 6: Commit Task 2**

```bash
git add src/round/hud.rs
git commit -m "feat: add companion gauge hud helpers"
```

## Task 3: Native macOS Companion Painter

**Files:**
- Modify: `src/companion/app.rs`

**Interfaces:**
- Consumes: `perimeter_gauge_layout`, `perimeter_gauge_colors`, `daily_fraction_for_gauge`, `companion_pace_fraction`, `companion_hud_text`.
- Produces: Three round-capped AppKit gauge lanes and a fixed three-line text stack.

- [ ] **Step 1: Add a failing unit assertion for the companion HUD stack**

Replace `companion_rate_stack_starts_at_legacy_subline_scale` with this test in `src/companion/app.rs`:

```rust
    #[test]
    fn companion_hud_stack_uses_daily_percent_and_drops_hour_rate() {
        let text = crate::round::hud::companion_hud_text(842_000_000.0, Some(0.94), 31_000_000.0);

        assert_eq!(text.today_total, "842M");
        assert_eq!(text.daily_percent, "94% yday");
        assert_eq!(text.pace, "31M/10m");
        assert!(!text.pace.contains("/hr"));
    }
```

- [ ] **Step 2: Run the companion test and confirm it fails before imports/rendering are changed**

Run:

```bash
cargo test companion_hud_stack_uses_daily_percent_and_drops_hour_rate -- --nocapture
```

Expected: FAIL until Task 2 has landed and this file calls the new helper.

- [ ] **Step 3: Replace the single growth-ring import and gap constant**

In `src/companion/app.rs`, replace:

```rust
use crate::round::hud::{growth_ring_fill_end_deg, growth_ring_layout};
```

with:

```rust
use crate::round::hud::{
    companion_hud_text, companion_pace_fraction, daily_fraction_for_gauge,
    growth_ring_fill_end_deg, perimeter_gauge_colors, perimeter_gauge_layout,
    CompanionHudText, GaugeLane, GaugeLaneColors, LineCap, COMPANION_GAUGE_GAP_DEG,
};
```

Add these AppKit imports:

```rust
    NSButtLineCapStyle, NSLineCapStyle, NSRoundLineCapStyle,
```

Remove `const COMPANION_RING_GAP_DEG: f64 = 70.0;` and use `COMPANION_GAUGE_GAP_DEG` in this file.

- [ ] **Step 4: Add AppKit lane drawing helpers**

Add these helpers above `draw_hud`:

```rust
#[cfg(target_os = "macos")]
fn ns_line_cap(cap: LineCap) -> NSLineCapStyle {
    match cap {
        LineCap::Butt => NSButtLineCapStyle,
        LineCap::Round => NSRoundLineCapStyle,
    }
}

#[cfg(target_os = "macos")]
fn draw_gauge_lane(lane: &GaugeLane, colors: &GaugeLaneColors, fraction: f64) {
    let start = lane.ring.track_start_deg;
    let end = lane.ring.track_start_deg + lane.ring.track_sweep_deg;

    unsafe {
        let track = NSBezierPath::new();
        track.setLineWidth(lane.stroke_width);
        track.setLineCapStyle(ns_line_cap(lane.cap));
        track.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle(
            NSPoint::new(lane.ring.cx, lane.ring.cy),
            lane.ring.radius,
            start,
            end,
        );
        ns_color(&colors.track).setStroke();
        track.stroke();

        let clamped = fraction.clamp(0.0, 1.0);
        if clamped > 0.0 {
            let fill_end = growth_ring_fill_end_deg(&lane.ring, clamped);
            let fill = NSBezierPath::new();
            fill.setLineWidth(lane.stroke_width);
            fill.setLineCapStyle(ns_line_cap(lane.cap));
            fill.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle(
                NSPoint::new(lane.ring.cx, lane.ring.cy),
                lane.ring.radius,
                start,
                fill_end,
            );
            ns_color(&colors.fill).setStroke();
            fill.stroke();
        }
    }
}
```

- [ ] **Step 5: Replace the single ring block with three lanes**

Replace the existing `// Growth ring (open-bottom arc).` block with:

```rust
        // Companion perimeter gauges: XP, today vs yesterday, and live 10m pace.
        {
            let cx = aperture.center_x as f64;
            let cy = aperture.center_y as f64;
            let layout = perimeter_gauge_layout(
                cx,
                cy,
                aperture.radius as f64,
                COMPANION_GAUGE_GAP_DEG,
            );
            let colors = perimeter_gauge_colors();
            let xp_fraction = if vm.progress.is_max_stage {
                1.0
            } else {
                vm.progress.fraction as f64
            };
            let daily_fraction =
                daily_fraction_for_gauge(vm.daily_comparison.fraction_of_yesterday);
            let pace_fraction = companion_pace_fraction(vm.rate_momentum.pulse.current_tokens);

            draw_gauge_lane(&layout.xp, &colors.xp, xp_fraction);
            draw_gauge_lane(&layout.daily, &colors.daily, daily_fraction);
            draw_gauge_lane(&layout.pace, &colors.pace, pace_fraction);
        }
```

- [ ] **Step 6: Replace HUD text measurement with a three-line stack**

In `draw_hud`, use the inner gauge lane for `stat_gap_box` and render all lines from one `CompanionHudText`:

```rust
    let gauge_layout = crate::round::hud::perimeter_gauge_layout(
        aperture.center_x as f64,
        aperture.center_y as f64,
        aperture.radius as f64,
        COMPANION_GAUGE_GAP_DEG,
    );
    let gap = crate::round::hud::stat_gap_box(
        aperture.center_x as f64,
        aperture.center_y as f64,
        gauge_layout.pace.ring.radius - gauge_layout.pace.stroke_width / 2.0,
        COMPANION_GAUGE_GAP_DEG,
    );

    let hud_text = companion_hud_text(
        vm.today_effective_tokens,
        vm.daily_comparison.fraction_of_yesterday,
        vm.rate_momentum.pulse.current_tokens,
    );
```

Replace the separate big/rate drawing with this stack loop:

```rust
        let big_color = RoundColor(0.93, 0.93, 0.97, 1.0);
        let sub_color = crate::round::hud::rate_direction_color(
            crate::tui::view_model::RateDirection::Neutral,
        );
        let mut stack_size = font_size * 1.45;
        let mut rendered = companion_hud_attributed_lines(&hud_text, stack_size, &big_color, &sub_color);

        while (rendered.max_width > gap.max_width || rendered.total_height > aperture.radius as f64 * 0.34)
            && stack_size > 6.0
        {
            stack_size -= 1.0;
            rendered = companion_hud_attributed_lines(&hud_text, stack_size, &big_color, &sub_color);
        }

        let top = bounds.size.height - gap.baseline_y;
        let mut y = top + rendered.total_height * 0.38;
        for line in rendered.lines {
            let width = line.text.size().width;
            line.text.drawAtPoint(NSPoint::new(gap.center_x - width / 2.0, y));
            y -= line.text.size().height * 0.82;
        }
```

Add the helper structs and function used by the loop:

```rust
#[cfg(target_os = "macos")]
struct CompanionAttributedLine {
    text: Retained<NSMutableAttributedString>,
}

#[cfg(target_os = "macos")]
struct CompanionAttributedStack {
    lines: Vec<CompanionAttributedLine>,
    max_width: f64,
    total_height: f64,
}

#[cfg(target_os = "macos")]
fn companion_hud_attributed_lines(
    text: &CompanionHudText,
    size: f64,
    big_color: &RoundColor,
    sub_color: &RoundColor,
) -> CompanionAttributedStack {
    let big = attributed_pet_glyph(&text.today_total, size * 1.08, big_color);
    let daily = attributed_pet_glyph(&text.daily_percent, size * 0.68, sub_color);
    let pace = attributed_pet_glyph(&text.pace, size * 0.68, sub_color);
    let max_width = big
        .size()
        .width
        .max(daily.size().width)
        .max(pace.size().width);
    let total_height = big.size().height + daily.size().height * 0.82 + pace.size().height * 0.82;

    CompanionAttributedStack {
        lines: vec![
            CompanionAttributedLine { text: big },
            CompanionAttributedLine { text: daily },
            CompanionAttributedLine { text: pace },
        ],
        max_width,
        total_height,
    }
}
```

- [ ] **Step 7: Run Task 3 checks**

Run:

```bash
cargo test companion_hud_stack_uses_daily_percent_and_drops_hour_rate -- --nocapture
cargo test --lib round::hud -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit Task 3**

```bash
git add src/companion/app.rs
git commit -m "feat: render companion perimeter gauges"
```

## Task 4: Preview Lab HUD Artifact

**Files:**
- Modify: `src/dev_preview/contract.rs`
- Modify: `src/dev_preview/export.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Modify: `src/dev_preview/round.rs`
- Test: `tests/dev_preview.rs`

**Interfaces:**
- Consumes: `PerimeterGaugeLayout`, `perimeter_gauge_colors`, `daily_fraction_for_gauge`, `companion_pace_fraction`, `companion_hud_text`.
- Produces: `PreviewFrameContract.hud: Option<PreviewHudArtifact>`.
- Produces: `frames/<id>.hud.json`.
- Produces: `PreviewScenarioFiles.hud: Option<PathBuf>`.
- Produces: artifact type `hud`.

- [ ] **Step 1: Write failing Preview Lab tests**

Add this helper near `read_scene` in `tests/dev_preview.rs`:

```rust
fn read_hud(run: &PreviewRun, id: &str) -> Value {
    read_json(run.out.join(format!("frames/{id}.hud.json")))
}
```

Add these tests near the round Preview Lab tests:

```rust
#[test]
fn dev_preview_round_writes_companion_hud_artifacts() {
    let run = PreviewRun::new();

    run.run_success("round");

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 6);
    let expected = [
        "round-normal",
        "round-hud-missing-yesterday",
        "round-hud-stale-yesterday",
        "round-hud-zero-yesterday",
        "round-hud-over-yesterday",
        "round-hud-idle-pace",
        "round-hud-burst-pace",
    ];

    for id in expected {
        assert!(
            run.out.join(format!("frames/{id}.hud.json")).is_file(),
            "missing {id}.hud.json"
        );
        let scenario = scenario(&manifest, id);
        assert_eq!(scenario["files"]["hud"], format!("frames/{id}.hud.json"));
        assert_artifact_type(&manifest, &format!("{id}-hud"), "hud");

        let hud = read_hud(&run, id);
        assert_eq!(hud["schema_version"], 1);
        assert_eq!(hud["frame_id"], id);
        assert_eq!(hud["gap_deg"], 70.0);
        assert_eq!(hud["lanes"]["xp"]["cap"], "round");
        assert_eq!(hud["lanes"]["daily"]["cap"], "round");
        assert_eq!(hud["lanes"]["pace"]["cap"], "round");
        assert!(hud["lanes"]["xp"]["stroke_width"].as_f64().unwrap() > hud["lanes"]["daily"]["stroke_width"].as_f64().unwrap());
        assert!(hud["lanes"]["daily"]["stroke_width"].as_f64().unwrap() > hud["lanes"]["pace"]["stroke_width"].as_f64().unwrap());
        assert!(hud["text"]["today_total"].is_string());
        assert!(hud["text"]["daily_percent"].as_str().unwrap().ends_with(" yday"));
        assert!(hud["text"]["pace"].as_str().unwrap().ends_with("/10m"));
    }
}

#[test]
fn dev_preview_hud_artifacts_cover_daily_and_pace_states() {
    let run = PreviewRun::new();

    run.run_success("round");

    let missing = read_hud(&run, "round-hud-missing-yesterday");
    assert_eq!(missing["lanes"]["daily"]["fill_fraction"], 0.0);
    assert_eq!(missing["text"]["daily_percent"], "--% yday");

    let zero = read_hud(&run, "round-hud-zero-yesterday");
    assert_eq!(zero["lanes"]["daily"]["fill_fraction"], 0.0);
    assert_eq!(zero["text"]["daily_percent"], "--% yday");

    let over = read_hud(&run, "round-hud-over-yesterday");
    assert_eq!(over["lanes"]["daily"]["fill_fraction"], 1.0);
    assert_eq!(over["text"]["daily_percent"], "124% yday");

    let idle = read_hud(&run, "round-hud-idle-pace");
    assert_eq!(idle["lanes"]["pace"]["fill_fraction"], 0.0);
    assert_eq!(idle["text"]["pace"], "0/10m");

    let burst = read_hud(&run, "round-hud-burst-pace");
    assert!(
        burst["lanes"]["pace"]["fill_fraction"].as_f64().unwrap() > 0.80,
        "burst pace should visibly fill the amber lane"
    );
}

#[test]
fn dev_preview_scene_artifacts_do_not_gain_companion_hud_metrics() {
    let run = PreviewRun::new();

    run.run_success("round");

    let scene_text =
        std::fs::read_to_string(run.out.join("frames/round-hud-over-yesterday.scene.json"))
            .unwrap();
    for forbidden in ["daily_comparison", "fraction_of_yesterday", "124% yday", "/10m", "842M"] {
        assert!(
            !scene_text.contains(forbidden),
            "scene artifact leaked companion HUD metric {forbidden}: {scene_text}"
        );
    }
}
```

- [ ] **Step 2: Run the Preview Lab tests and confirm they fail on missing HUD artifacts**

Run:

```bash
cargo test --features dev-preview --test dev_preview dev_preview_round_writes_companion_hud_artifacts -- --nocapture
```

Expected: FAIL because `schema_version` is still `5` and `frames/<id>.hud.json` is not written.

- [ ] **Step 3: Add the HUD artifact contract**

In `src/dev_preview/contract.rs`, extend `PreviewFrameContract`:

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreviewFrameContract {
    pub scene: Option<PreviewSceneArtifact>,
    pub hud: Option<PreviewHudArtifact>,
}
```

Add these serializable structs:

```rust
pub const HUD_CONTRACT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewHudArtifact {
    pub schema_version: u32,
    pub frame_id: String,
    pub gap_deg: f64,
    pub aperture_radius: f64,
    pub lanes: BTreeMap<String, PreviewHudLaneArtifact>,
    pub text: PreviewHudTextArtifact,
    pub privacy_projection: PreviewPrivacyProjection,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewHudLaneArtifact {
    pub radius: f64,
    pub stroke_width: f64,
    pub track_start_deg: f64,
    pub track_sweep_deg: f64,
    pub fill_fraction: f64,
    pub cap: String,
    pub track_color: PreviewHudColorArtifact,
    pub fill_color: PreviewHudColorArtifact,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PreviewHudTextArtifact {
    pub today_total: String,
    pub daily_percent: String,
    pub pace: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct PreviewHudColorArtifact {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}
```

Add the constructor:

```rust
impl PreviewHudArtifact {
    pub fn from_companion_view_model(
        frame_id: &str,
        vm: &WatchViewModel,
        aperture: crate::round::layout::RoundAperture,
    ) -> Self {
        let gap_deg = crate::round::hud::COMPANION_GAUGE_GAP_DEG;
        let layout = crate::round::hud::perimeter_gauge_layout(
            aperture.center_x as f64,
            aperture.center_y as f64,
            aperture.radius as f64,
            gap_deg,
        );
        let colors = crate::round::hud::perimeter_gauge_colors();
        let xp_fraction = if vm.progress.is_max_stage {
            1.0
        } else {
            vm.progress.fraction as f64
        };
        let daily_fraction =
            crate::round::hud::daily_fraction_for_gauge(vm.daily_comparison.fraction_of_yesterday);
        let pace_fraction =
            crate::round::hud::companion_pace_fraction(vm.rate_momentum.pulse.current_tokens);
        let text = crate::round::hud::companion_hud_text(
            vm.today_effective_tokens,
            vm.daily_comparison.fraction_of_yesterday,
            vm.rate_momentum.pulse.current_tokens,
        );

        Self {
            schema_version: HUD_CONTRACT_SCHEMA_VERSION,
            frame_id: frame_id.to_string(),
            gap_deg,
            aperture_radius: aperture.radius as f64,
            lanes: BTreeMap::from([
                (
                    "xp".to_string(),
                    PreviewHudLaneArtifact::from_lane(&layout.xp, &colors.xp, xp_fraction),
                ),
                (
                    "daily".to_string(),
                    PreviewHudLaneArtifact::from_lane(&layout.daily, &colors.daily, daily_fraction),
                ),
                (
                    "pace".to_string(),
                    PreviewHudLaneArtifact::from_lane(&layout.pace, &colors.pace, pace_fraction),
                ),
            ]),
            text: PreviewHudTextArtifact {
                today_total: text.today_total,
                daily_percent: text.daily_percent,
                pace: text.pace,
            },
            privacy_projection: PreviewPrivacyProjection {
                surface: "companion-hud".to_string(),
                source_names_visible: false,
                exact_counts_visible: true,
                diagnostic_text_visible: false,
                feed_rows_visible: false,
                file_paths_visible: false,
                project_names_visible: false,
            },
        }
    }
}

impl PreviewHudLaneArtifact {
    fn from_lane(
        lane: &crate::round::hud::GaugeLane,
        colors: &crate::round::hud::GaugeLaneColors,
        fill_fraction: f64,
    ) -> Self {
        Self {
            radius: lane.ring.radius,
            stroke_width: lane.stroke_width,
            track_start_deg: lane.ring.track_start_deg,
            track_sweep_deg: lane.ring.track_sweep_deg,
            fill_fraction: fill_fraction.clamp(0.0, 1.0),
            cap: match lane.cap {
                crate::round::hud::LineCap::Butt => "butt".to_string(),
                crate::round::hud::LineCap::Round => "round".to_string(),
            },
            track_color: PreviewHudColorArtifact::from_round_color(colors.track),
            fill_color: PreviewHudColorArtifact::from_round_color(colors.fill),
        }
    }
}

impl PreviewHudColorArtifact {
    fn from_round_color(color: crate::round::draw::RoundColor) -> Self {
        Self {
            r: color.0,
            g: color.1,
            b: color.2,
            a: color.3,
        }
    }
}
```

- [ ] **Step 4: Add HUD manifest and export plumbing**

In `src/dev_preview/export.rs`, set schema version to `6`:

```rust
pub const SCHEMA_VERSION: u32 = 6;
```

Add `hud` to `PreviewScenarioFiles`:

```rust
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hud: Option<PathBuf>,
```

Add `Hud` to `ArtifactType`:

```rust
    Hud,
```

In `write_review_markdown`, add:

```rust
            if let Some(hud) = &scenario.files.hud {
                markdown.push_str(&format!("- HUD: `{}`\n", hud.display()));
            }
```

In `src/dev_preview/scenarios.rs`, write HUD artifacts in the frame loop:

```rust
        if let Some(hud) = &frame.contract.hud {
            write_json_artifact(&staging_dir.join(hud_path(frame)), hud)?;
        }
```

Add `hud` to `PreviewScenarioFiles`:

```rust
            hud: frame.contract.hud.as_ref().map(|_| hud_path(frame)),
```

Add HUD artifact entries:

```rust
        if frame.contract.hud.is_some() {
            artifacts.push(PreviewArtifact {
                id: format!("{}-hud", frame.id),
                title: format!("{} HUD", frame.title),
                artifact_type: ArtifactType::Hud,
                path: hud_path(frame),
                width: None,
                height: None,
            });
        }
```

Add the path helper:

```rust
fn hud_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.hud.json", frame.id))
}
```

- [ ] **Step 5: Attach HUD artifacts and fixture variants to round preview**

In `src/dev_preview/round.rs`, modify `frame` to attach a HUD artifact:

```rust
fn frame(
    id: &str,
    title: &str,
    vm: &WatchViewModel,
    ctx: &PreviewRenderContext,
    capabilities: RoundRenderCapabilities,
) -> PreviewFrame {
    let mut frame = render_round_preview_frame_from_vm(id, title, vm, ctx.fixed_now, 52, 52, capabilities);
    let aperture = RoundAperture::new(frame.width, frame.height);
    frame.contract.hud = Some(
        crate::dev_preview::contract::PreviewHudArtifact::from_companion_view_model(
            &frame.id,
            vm,
            aperture,
        ),
    );
    frame
}
```

Add this helper in `src/dev_preview/round.rs`:

```rust
fn set_daily_comparison(
    vm: &mut WatchViewModel,
    today_tokens: f64,
    yesterday_tokens: Option<f64>,
    yesterday_state: crate::usage::snapshot::SnapshotState,
    reason: Option<&str>,
) {
    vm.today_effective_tokens = today_tokens;
    vm.daily_comparison = crate::tui::view_model::DailyComparison {
        today_provider_day: time::macros::date!(2026 - 07 - 06),
        yesterday_provider_day: time::macros::date!(2026 - 07 - 05),
        today_tokens,
        yesterday_tokens,
        today_snapshot_state: crate::usage::snapshot::SnapshotState::Current,
        yesterday_snapshot_state: yesterday_state,
        today_observed_at: Some(time::macros::datetime!(2026 - 07 - 06 20:00 UTC)),
        yesterday_observed_at: Some(time::macros::datetime!(2026 - 07 - 05 20:00 UTC)),
        unavailable_reason: reason.map(str::to_string),
        fraction_of_yesterday: match (yesterday_tokens, reason) {
            (Some(yesterday), None) if yesterday > 0.0 => Some(today_tokens / yesterday),
            _ => None,
        },
    };
}
```

Add these variants to `round_frames` after `round-normal`:

```rust
    let mut missing_yesterday = WatchViewModel::fixture_with_habitat_props();
    set_daily_comparison(
        &mut missing_yesterday,
        842_000_000.0,
        None,
        crate::usage::snapshot::SnapshotState::Missing,
        Some("yesterday-missing"),
    );
    frames.push(frame(
        "round-hud-missing-yesterday",
        "Round HUD Missing Yesterday",
        &missing_yesterday,
        ctx,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut stale_yesterday = WatchViewModel::fixture_with_habitat_props();
    set_daily_comparison(
        &mut stale_yesterday,
        842_000_000.0,
        Some(900_000_000.0),
        crate::usage::snapshot::SnapshotState::Stale,
        Some("yesterday-stale"),
    );
    frames.push(frame(
        "round-hud-stale-yesterday",
        "Round HUD Stale Yesterday",
        &stale_yesterday,
        ctx,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut zero_yesterday = WatchViewModel::fixture_with_habitat_props();
    set_daily_comparison(
        &mut zero_yesterday,
        842_000_000.0,
        Some(0.0),
        crate::usage::snapshot::SnapshotState::Current,
        Some("yesterday-zero"),
    );
    frames.push(frame(
        "round-hud-zero-yesterday",
        "Round HUD Zero Yesterday",
        &zero_yesterday,
        ctx,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut over_yesterday = WatchViewModel::fixture_with_habitat_props();
    set_daily_comparison(
        &mut over_yesterday,
        842_000_000.0,
        Some(678_000_000.0),
        crate::usage::snapshot::SnapshotState::Current,
        None,
    );
    frames.push(frame(
        "round-hud-over-yesterday",
        "Round HUD Over Yesterday",
        &over_yesterday,
        ctx,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut idle_pace = WatchViewModel::fixture_with_habitat_props();
    idle_pace.rate_momentum.pulse.current_tokens = 0.0;
    frames.push(frame(
        "round-hud-idle-pace",
        "Round HUD Idle Pace",
        &idle_pace,
        ctx,
        RoundRenderCapabilities::preview_truecolor(),
    ));

    let mut burst_pace = WatchViewModel::fixture_with_habitat_props();
    burst_pace.rate_momentum.pulse.current_tokens = 100_000_000.0;
    frames.push(frame(
        "round-hud-burst-pace",
        "Round HUD Burst Pace",
        &burst_pace,
        ctx,
        RoundRenderCapabilities::preview_truecolor(),
    ));
```

- [ ] **Step 6: Update existing schema assertions and round ID lists**

In `tests/dev_preview.rs`, change existing schema expectations from `5` to `6`.

Extend `ROUND_IDS` with:

```rust
    "round-hud-missing-yesterday",
    "round-hud-stale-yesterday",
    "round-hud-zero-yesterday",
    "round-hud-over-yesterday",
    "round-hud-idle-pace",
    "round-hud-burst-pace",
```

Update the full `scenario_ids` expectation to include those IDs immediately after `"round-normal"`.

- [ ] **Step 7: Run Task 4 tests**

Run:

```bash
cargo test --features dev-preview --test dev_preview dev_preview_round_writes_companion_hud_artifacts -- --nocapture
cargo test --features dev-preview --test dev_preview dev_preview_hud_artifacts_cover_daily_and_pace_states -- --nocapture
cargo test --features dev-preview --test dev_preview dev_preview_scene_artifacts_do_not_gain_companion_hud_metrics -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit Task 4**

```bash
git add src/dev_preview/contract.rs src/dev_preview/export.rs src/dev_preview/scenarios.rs src/dev_preview/round.rs tests/dev_preview.rs
git commit -m "feat: add companion hud preview artifacts"
```

## Task 5: Privacy Guard And Full Verification

**Files:**
- Modify: `tests/round_scene.rs`

**Interfaces:**
- Consumes: `derive_round_scene_model(vm: &WatchViewModel, now: OffsetDateTime) -> RoundSceneModel`.
- Produces: A privacy regression test proving companion HUD exact metrics stay outside `RoundSceneModel`.

- [ ] **Step 1: Add the privacy regression test**

Add this test to `tests/round_scene.rs`:

```rust
#[test]
fn round_scene_model_does_not_carry_companion_hud_metrics() {
    let now = datetime!(2026 - 07 - 06 20:00 UTC);
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.today_effective_tokens = 842_000_000.0;
    vm.rate_momentum.pulse.current_tokens = 31_000_000.0;
    vm.daily_comparison = glorp::tui::view_model::DailyComparison {
        today_provider_day: time::macros::date!(2026 - 07 - 06),
        yesterday_provider_day: time::macros::date!(2026 - 07 - 05),
        today_tokens: 842_000_000.0,
        yesterday_tokens: Some(678_000_000.0),
        today_snapshot_state: glorp::usage::snapshot::SnapshotState::Current,
        yesterday_snapshot_state: glorp::usage::snapshot::SnapshotState::Current,
        today_observed_at: Some(now),
        yesterday_observed_at: Some(now - time::Duration::days(1)),
        unavailable_reason: None,
        fraction_of_yesterday: Some(842_000_000.0 / 678_000_000.0),
    };

    let scene = glorp::round::model::derive_round_scene_model(&vm, now);
    let debug = format!("{scene:#?}");

    for forbidden in [
        "daily_comparison",
        "fraction_of_yesterday",
        "842000000",
        "31000000",
        "124% yday",
        "/10m",
    ] {
        assert!(
            !debug.contains(forbidden),
            "RoundSceneModel leaked companion HUD metric {forbidden}: {debug}"
        );
    }
}
```

- [ ] **Step 2: Run the privacy test**

Run:

```bash
cargo test --test round_scene round_scene_model_does_not_carry_companion_hud_metrics -- --nocapture
```

Expected: PASS.

- [ ] **Step 3: Run focused feature verification**

Run:

```bash
cargo test --test watch_integration daily_comparison_ -- --nocapture
cargo test --lib round::hud -- --nocapture
cargo test --test round_scene -- --nocapture
cargo test --features dev-preview --test dev_preview dev_preview_round_writes_companion_hud_artifacts -- --nocapture
cargo test --features dev-preview --test dev_preview dev_preview_hud_artifacts_cover_daily_and_pace_states -- --nocapture
cargo test --features dev-preview --test dev_preview dev_preview_scene_artifacts_do_not_gain_companion_hud_metrics -- --nocapture
```

Expected: all commands PASS.

- [ ] **Step 4: Run full requested checks**

Run:

```bash
cargo test --test watch_integration
cargo test --test round_scene
cargo test --features dev-preview --test dev_preview
cargo test --lib round::hud
cargo xtask companion fresh
```

Expected: all tests PASS. `cargo xtask companion fresh` builds and opens `target/macos/Glorp.app`.

- [ ] **Step 5: Generate deterministic preview bundle for visual review**

Run:

```bash
cargo run -- dev-preview --scenario round --out target/glorp-preview
open target/glorp-preview/index.html
```

Expected: `target/glorp-preview/manifest.json` has `schema_version: 6`, each round scenario has `files.hud`, and `frames/round-hud-over-yesterday.hud.json` shows `daily.fill_fraction` as `1.0` with text `124% yday`.

- [ ] **Step 6: Commit Task 5**

```bash
git add tests/round_scene.rs
git commit -m "test: guard companion hud privacy boundary"
```

## Final Integration Review

- [ ] Confirm `git status --short` shows only Drew's unrelated dirty files if they remain: `src/storage/usage_store.rs` and `tests/storage_privacy.rs`.
- [ ] Confirm no companion HUD path prints `/hr` by running `rg -n "/hr|rate_per_hour" src/companion src/round tests/dev_preview.rs tests/round_scene.rs`.
- [ ] Confirm no exact HUD field exists in `src/round/model.rs` by running `rg -n "daily_comparison|fraction_of_yesterday|today_effective_tokens|rate_momentum" src/round/model.rs`.
- [ ] Confirm Preview Lab manifest schema expectations are all `6` by running `rg -n "schema_version\"\\], 5|SCHEMA_VERSION: u32 = 5" src tests`.
- [ ] Confirm formatting with `cargo fmt --check`.

## Self-Review Notes

- Spec coverage: Task 1 covers provider-day daily comparison; Task 2 covers geometry, colors, caps, pace normalization, and text formatting; Task 3 covers AppKit rendering; Task 4 covers Preview Lab HUD artifacts and daily/pace fixtures; Task 5 covers the `RoundSceneModel` privacy boundary and verification.
- Filler scan: run the required no-filler-token search against this file. The expected result is no matches.
- Type consistency: the task interfaces use `DailyComparison`, `PerimeterGaugeLayout`, `GaugeLane`, `LineCap`, `CompanionHudText`, and `PreviewHudArtifact` with the same names across model, renderer, preview, and tests.
