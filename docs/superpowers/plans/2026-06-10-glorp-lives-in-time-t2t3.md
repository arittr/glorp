# Glorp Lives In Time — Combined T2+T3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the rest of the lives-in-time roadmap as one branch: the usage discontinuity guard, local feed timestamps, the pet's day (motes, tiredness, morning-after, dreams, the binding speech precedence stack), and day character + slow change (sky character, prop resonance, weekend texture, climate, seasons).

**Spec:** `docs/superpowers/specs/2026-06-09-glorp-lives-in-time-design.md` — Amendment (2026-06-10) + Branch T2 + Branch T3 sections. Branch: `feat/lives-in-time-t2t3`.

**Architecture:** Everything consumes the shipped T1 `DayContext` (`src/tui/day.rs`) and `LocalDayMapper` (`src/storage/day_axis.rs`). Two new `DayContext` fields (`tiredness`, `local_day_started_utc`), one feeding-semantics change (the guard inside `stage_usage_poll_deltas` — the only chokepoint covering watch, menubar, and `glorp status`), and otherwise presentation-layer consumers. No new persisted semantic state; one new `AppConfig` field (`discontinuity_guard_ratio`, serde default).

**Tech Stack:** Rust, rusqlite 0.32, time 0.3, ratatui, insta, Preview Lab.

**House rules binding every task:** TDD (failing test → red → implement → green); `cargo fmt` + `cargo clippy --all-targets --all-features -- -D warnings` before every commit; guard and sleep tests drive the REAL production paths (`stage_usage_poll_deltas`, never hand-built internals); live smoke ONLY against an isolated `GLORP_CONFIG_DIR` — running an unverified binary against the real pet is how the 2026-06-10 bolus happened.

**Interface sheet (binding):** the shared types, constants, signatures, wiring rules, and honesty invariants for all tasks were fixed before authoring and are reproduced in the Appendix at the bottom of this plan. Where a task's code conflicts with the appendix, the appendix wins — flag the conflict instead of improvising.

---

## File structure

| File | Status | Responsibility |
|---|---|---|
| `src/game/runtime.rs` | modify | discontinuity guard inside `stage_usage_poll_deltas` (signature change), constants |
| `src/storage/usage_store.rs` | modify | `latest_cursor_updated_at`, `refuse_poll_discontinuity` (one tx) |
| `src/config.rs` | modify | `discontinuity_guard_ratio` field (serde default 5.0) |
| `src/commands/status.rs`, `src/commands/watch.rs` | modify | guard threading, surfacing exemptions, local timestamps |
| `src/tui/day.rs` | modify | `tiredness`, `local_day_started_utc`, `in_morning_after_window`, `resonant_prop_for_day`, `weekend_softening` |
| `src/pet/speech.rs` | modify | full precedence stack, dreams, morning-after |
| `src/pet/render.rs` | modify | `AnimationFrame.blink_slowdown` |
| `src/pet/animator.rs` | modify | `BreathRhythm::Tired` |
| `src/pet/activity.rs` / `src/pet/narration.rs` | modify | `format_hhmm_local`, refusal narrative |
| `src/tui/panels/pet.rs` | modify | motes pass, sky character/climate/seasons, weekend softening, resonance styling |
| `src/tui/app.rs` / `src/menubar/app.rs` | modify | speech/breath/blink caller updates |
| `src/dev_preview/` + `tests/dev_preview.rs` | modify | seven new fixtures, ordered pins, one new snapshot |

Task map: 1–3 guard + plumbing · 4–6 the creature (tiredness, voice, motion) · 7–9 the scene (motes, sky/climate/seasons, weekend) · 10–12 resonance + preview proof + final gate.

<!-- STITCH: parts a, b, c, d follow; appendix (interface sheet) last -->
## Section A — Tasks 1–3: usage discontinuity guard + plumbing

**Spec sections:** "Amendment (2026-06-10): T2 + T3 ship as one combined branch" (both items: the usage discontinuity guard and local feed timestamps), plus the Honesty rules. Binding contracts: the T2+T3 interface sheet — `stage_usage_poll_deltas` signature change, `UsageStore::latest_cursor_updated_at` / `refuse_poll_discontinuity`, `DISCONTINUITY_GUARD_RATIO = 5.0` (config-overridable via `discontinuity_guard_ratio`), `DISCONTINUITY_GUARD_FLOOR_TOKENS = 50_000_000.0`, `format_hhmm_local(now, offset)`, and the surfacing exemptions for the `usage_discontinuity` diagnostic code.

**Honesty invariants these tasks own:** refused tokens are never retro-fed (the config ratio is the escape hatch going forward); a refusal narrates once ("{pet name} declined an implausible feast") and stamps `last_idle_narration_at` so the same pass cannot also narrate boredom; a refused source is never displayed as broken/blocked — the source is healthy, one poll was refused; timestamps are display-only local conversions, no stored data changes.

**Files touched in this section:**

| File | Status | Responsibility |
|---|---|---|
| `src/storage/usage_store.rs` | modify | `latest_cursor_updated_at`, `refuse_poll_discontinuity` (one transaction), shared cursor-upsert helper |
| `src/config.rs` | modify | `discontinuity_guard_ratio` field with serde default |
| `src/game/runtime.rs` | modify | guard constants, `USAGE_DISCONTINUITY_CODE`, guard inside `stage_usage_poll_deltas`, signature change, wrapper update |
| `src/commands/status.rs` | modify | guard-threading call site; store-read surfacing without claiming blocked |
| `src/commands/watch.rs` | modify | guard-threading call site; source_health/active_diagnostics exemptions; local timestamps; in-module test callers |
| `src/pet/activity.rs` | modify | `format_hhmm_local`; offset params on both activity derivers |
| `src/tui/app.rs` | modify | harness `timestamp_column` routes through `format_hhmm_local`; profile-activity offset |
| `tests/runtime_integration.rs` | modify | guard tests; mechanical caller migration (contact seeding + signatures) |
| `tests/usage_provider.rs` | modify | contact seeding for the staging-asserting test |
| `tests/doctor_status.rs` | modify | contact seeding; status discontinuity surfacing test |
| `tests/game_rules.rs` | modify | `AppConfig` struct literal gains `..AppConfig::default()` |

**Verbatim-anchor warning:** Read every target region before editing — `mark_events_applied_and_advance_cursors` (`src/storage/usage_store.rs:647-750`) and `advance_cursors` (`src/storage/usage_store.rs:399-436`) contain multi-line SQL string literals; copy `old_string` anchors from the live file, never from this plan or the extraction excerpts.

---

### Task 1: `UsageStore` discontinuity primitives — `latest_cursor_updated_at` + `refuse_poll_discontinuity`

**Spec section:** Amendment item 1 — "its cursors advance without staging rows and a `usage_discontinuity` diagnostic persists, both in **one transaction** (a crash between separate writes would discard tokens with no record)" and "`days_factor` = whole days since that provider's newest `provider_cursors.updated_at` + 1".

**Files:**
- Modify: `src/storage/usage_store.rs` (new methods after `advance_cursors` at `src/storage/usage_store.rs:399-436`; shared upsert helper; tests in the in-module `#[cfg(test)] mod tests` at `src/storage/usage_store.rs:1220`)

- [ ] **Step 1: Write the failing tests**

Add to the in-module tests mod at the bottom of `src/storage/usage_store.rs` (after the existing tests; `use super::*;` and `use time::macros::datetime;` are already in scope at `src/storage/usage_store.rs:1222-1224`):

```rust
    fn contact_update(surface: &str, key: &str) -> ProviderCursorUpdate {
        ProviderCursorUpdate {
            provider_surface: surface.to_string(),
            cursor_key: key.to_string(),
            cursor_value: "seeded".to_string(),
            provider_version: "test-provider".to_string(),
            parser_version: "test-parser".to_string(),
        }
    }

    #[test]
    fn latest_cursor_updated_at_is_none_without_cursors_and_max_per_surface() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        assert_eq!(store.latest_cursor_updated_at("claude-code").unwrap(), None);

        let early = datetime!(2026-06-08 09:00 UTC);
        let late = datetime!(2026-06-09 21:00 UTC);
        store
            .advance_cursors(vec![contact_update("claude-code", "cursor-a")], early)
            .unwrap();
        store
            .advance_cursors(vec![contact_update("claude-code", "cursor-b")], late)
            .unwrap();
        store
            .advance_cursors(vec![contact_update("codex", "cursor-c")], early)
            .unwrap();

        assert_eq!(
            store.latest_cursor_updated_at("claude-code").unwrap(),
            Some(late)
        );
        assert_eq!(
            store.latest_cursor_updated_at("codex").unwrap(),
            Some(early)
        );
        assert_eq!(store.latest_cursor_updated_at("gemini").unwrap(), None);
    }

    #[test]
    fn refuse_poll_discontinuity_advances_cursors_and_persists_diagnostic_together() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = datetime!(2026-06-10 08:00 UTC);
        let diagnostic = ProviderDiagnostic {
            provider_surface: "claude-code".to_string(),
            code: "usage_discontinuity".to_string(),
            message: "refused 212000000 effective tokens (threshold 99000000)".to_string(),
            recorded_at: now,
        };

        store
            .refuse_poll_discontinuity(
                vec![contact_update("claude-code", "cursor-a")],
                &diagnostic,
                now,
            )
            .unwrap();

        // Cursors advanced...
        assert_eq!(
            store.latest_cursor_updated_at("claude-code").unwrap(),
            Some(now)
        );
        assert_eq!(
            store
                .provider_cursor("claude-code", "cursor-a")
                .unwrap()
                .as_deref(),
            Some("seeded")
        );
        // ...the diagnostic persisted...
        let stored = store.recent_diagnostics(5).unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].provider_surface, "claude-code");
        assert_eq!(stored[0].code, "usage_discontinuity");
        assert_eq!(stored[0].recorded_at, now);
        // ...and nothing was staged: a refusal never creates food.
        assert_eq!(store.unapplied_events(10).unwrap().len(), 0);
        assert_eq!(store.recent_event_count().unwrap(), 0);
    }

    #[test]
    fn refuse_poll_discontinuity_is_one_transaction() {
        // Atomicity: the diagnostic is present iff the cursors advanced. Drop
        // the diagnostics table so the LAST statement in the transaction
        // fails — a non-transactional implementation would leave the cursor
        // upserts behind, silently discarding tokens with no record.
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = datetime!(2026-06-10 08:00 UTC);
        store
            .conn
            .execute("DROP TABLE provider_diagnostics", [])
            .unwrap();
        let diagnostic = ProviderDiagnostic {
            provider_surface: "claude-code".to_string(),
            code: "usage_discontinuity".to_string(),
            message: "refused".to_string(),
            recorded_at: now,
        };

        let result = store.refuse_poll_discontinuity(
            vec![contact_update("claude-code", "cursor-a")],
            &diagnostic,
            now,
        );

        assert!(
            result.is_err(),
            "a failed diagnostic insert must fail the whole call"
        );
        assert_eq!(
            store.latest_cursor_updated_at("claude-code").unwrap(),
            None,
            "cursor upserts must roll back with the failed diagnostic insert"
        );
    }
```

- [ ] **Step 2: Run the tests, watch them fail**

Run: `cargo test --lib latest_cursor_updated_at` and `cargo test --lib refuse_poll_discontinuity`
Expected: compile error — `error[E0599]: no method named 'latest_cursor_updated_at' found for struct 'UsageStore'` (and the same for `refuse_poll_discontinuity`).

- [ ] **Step 3: Implement**

First extract the cursor-upsert statement into a shared free function. The identical SQL currently appears three times: `advance_cursors` (`src/storage/usage_store.rs:410-432`), `mark_events_applied_and_advance_cursors` (`src/storage/usage_store.rs:721-744`), and the new method would be a fourth. Read both regions first, then add this free function right after `advance_cursors`:

```rust
/// Upsert one provider cursor inside an existing transaction. Shared by
/// `advance_cursors`, `mark_events_applied_and_advance_cursors`, and
/// `refuse_poll_discontinuity` so the conflict-update column set cannot
/// drift between the three cursor-advance paths.
fn upsert_provider_cursor(
    tx: &rusqlite::Transaction<'_>,
    update: &ProviderCursorUpdate,
    updated_at: &str,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO provider_cursors (
            provider_surface,
            cursor_key,
            cursor_value,
            provider_version,
            parser_version,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(provider_surface, cursor_key) DO UPDATE SET
            cursor_value = excluded.cursor_value,
            provider_version = excluded.provider_version,
            parser_version = excluded.parser_version,
            updated_at = excluded.updated_at",
        params![
            update.provider_surface,
            update.cursor_key,
            update.cursor_value,
            update.provider_version,
            update.parser_version,
            updated_at,
        ],
    )?;
    Ok(())
}
```

(Note: `upsert_provider_cursor` is a module-level free function, not a method — it borrows the transaction, so it cannot take `&self` while a `self.conn.transaction()` is live.)

Replace the inline upsert loop in `advance_cursors` (Read `src/storage/usage_store.rs:399-436` first; the body between `let tx = self.conn.transaction()?;` and `tx.commit()?;` becomes):

```rust
        for update in &updates {
            upsert_provider_cursor(&tx, update, &updated_at)?;
        }
```

Replace the inline upsert loop in `mark_events_applied_and_advance_cursors` (Read `src/storage/usage_store.rs:720-745` first; the `for update in &pending_updates { tx.execute(...) }` loop becomes):

```rust
        for update in &pending_updates {
            upsert_provider_cursor(&tx, update, &applied_at_text)?;
        }
```

Then add the two new methods directly after `advance_cursors`:

```rust
    /// Newest `provider_cursors.updated_at` for one provider surface — the
    /// discontinuity guard's per-provider "last fed" instant. `None` means
    /// the surface has never had a cursor (first contact). MAX is computed
    /// lexically on the stored RFC3339 text; all writers share `format_time`,
    /// so orderings can differ only within a single second, which the
    /// guard's whole-day factor ignores.
    pub fn latest_cursor_updated_at(
        &self,
        provider_surface: &str,
    ) -> crate::error::Result<Option<OffsetDateTime>> {
        self.conn
            .query_row(
                "SELECT MAX(updated_at) FROM provider_cursors
                 WHERE provider_surface = ?1",
                params![provider_surface],
                |row| {
                    let raw: Option<String> = row.get(0)?;
                    raw.as_deref().map(parse_time_for_sql).transpose()
                },
            )
            .map_err(Into::into)
    }

    /// Refuse one provider surface's poll: advance its cursors WITHOUT
    /// staging any ledger rows and persist the refusal diagnostic, all in
    /// ONE transaction — a crash between separate writes would discard
    /// tokens with no record. Refused tokens are never retro-fed.
    pub fn refuse_poll_discontinuity(
        &mut self,
        updates: Vec<ProviderCursorUpdate>,
        diagnostic: &ProviderDiagnostic,
        now: OffsetDateTime,
    ) -> crate::error::Result<()> {
        let updated_at = format_time(now)?;
        let recorded_at = format_time(diagnostic.recorded_at)?;
        let tx = self.conn.transaction()?;
        for update in &updates {
            upsert_provider_cursor(&tx, update, &updated_at)?;
        }
        tx.execute(
            "INSERT INTO provider_diagnostics (
                provider_surface,
                code,
                message,
                recorded_at
            ) VALUES (?1, ?2, ?3, ?4)",
            params![
                diagnostic.provider_surface,
                diagnostic.code,
                diagnostic.message,
                recorded_at,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }
```

(`SELECT MAX(...)` over zero rows yields one row holding NULL, so `query_row` always finds a row and `None` falls out of the `Option<String>` — no `.optional()` needed. `parse_time_for_sql` is the existing helper at `src/storage/usage_store.rs:1215`.)

- [ ] **Step 4: Run the tests, verify they pass**

Run: `cargo test --lib latest_cursor_updated_at` then `cargo test --lib refuse_poll_discontinuity`
Expected: 1 passed, then 2 passed. Then `cargo test` — full suite green (the `advance_cursors` / `mark_events_applied_and_advance_cursors` refactor must not change behavior).

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add src/storage/usage_store.rs
git commit -m "feat(storage): add per-surface cursor recency and atomic poll refusal"
```

---

### Task 2: the discontinuity guard inside `stage_usage_poll_deltas`

**Spec section:** Amendment item 1, complete. The guard lives INSIDE `stage_usage_poll_deltas` — the only true chokepoint; `glorp status` carries its own inline copy of the poll pipeline (`src/commands/status.rs:31-47`), so any caller-side guard would leave it unguarded. Binding signature change from the interface sheet: the `baseline: CalibrationBaseline` param becomes `state: &mut PetState` (carries calibration + `recent_events` + `last_idle_narration_at`) and a `guard_ratio: f64` param is threaded from `AppConfig` by both poll paths.

Sanity check against the live incident (keep this arithmetic in mind reviewing the tests): baseline 19.77M, ratio 5.0, days_factor 1 → threshold = max(5 × 19.77M × 1, 50M) ≈ 99M ≪ the 212M bolus → fires at ~2× margin.

**Files:**
- Modify: `src/config.rs` (new `discontinuity_guard_ratio` field + serde default + in-module tests)
- Modify: `src/game/runtime.rs` (constants block at `src/game/runtime.rs:20-25`; `stage_usage_poll_deltas` at `src/game/runtime.rs:34-74`; `apply_usage_poll` wrapper at `src/game/runtime.rs:348-358`)
- Modify: `src/commands/status.rs:31-34` (guard-threading call site)
- Modify: `src/commands/watch.rs:393-395` (guard-threading call site) and the in-module test callers at `src/commands/watch.rs:1115` and `src/commands/watch.rs:1170`
- Test: `tests/runtime_integration.rs` (new guard tests + mechanical migration of existing callers), `tests/usage_provider.rs`, `tests/doctor_status.rs`, `tests/game_rules.rs`

- [ ] **Step 1: Write the failing tests**

Add to `tests/runtime_integration.rs`. First extend the runtime import (Read `tests/runtime_integration.rs:1-17`; the use list at line 4 gains the constant):

```rust
        runtime::{
            apply_unapplied_usage, apply_usage_poll, stage_usage_poll_deltas,
            DISCONTINUITY_GUARD_RATIO,
        },
```

Add the contact-seeding helper next to `empty_poll()` (`tests/runtime_integration.rs:267-273`). A surface with no cursors at all is first contact and is always refused, so every test that stages deltas must first simulate what `glorp init` does for providers present at init — advance one cursor:

```rust
fn establish_provider_contact(
    usage_store: &mut UsageStore,
    surface: &str,
    now: time::OffsetDateTime,
) {
    usage_store
        .advance_cursors(
            vec![ProviderCursorUpdate {
                provider_surface: surface.to_string(),
                cursor_key: format!("{surface}-first-contact"),
                cursor_value: "seeded".to_string(),
                provider_version: "test-provider".to_string(),
                parser_version: "test-parser".to_string(),
            }],
            now,
        )
        .unwrap();
}
```

Then add the four guard tests (these drive the REAL `stage_usage_poll_deltas` production path with `UsagePollResult` fixtures — never hand-built ledger internals):

```rust
#[test]
fn discontinuity_bolus_is_refused_alone_while_honest_sibling_feeds() {
    // The 2026-06-10 incident shape: ccusage 20.x silently became an
    // all-agents aggregator and fed a 212M-effective bolus on one poll while
    // codex fed honestly beside it. The offender is refused ALONE.
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 19_770_000.0; // live-incident median
    let now = datetime!(2026 - 06 - 10 08:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now - Duration::hours(1));
    establish_provider_contact(&mut usage_store, "codex", now - Duration::hours(1));

    let mut poll = poll_with_delta(212_000_000.0, now);
    poll.deltas
        .extend(poll_with_surface("codex", 40_000.0, now).deltas);
    poll.total_effective_tokens = 212_040_000.0;

    let ids = stage_usage_poll_deltas(
        &mut usage_store,
        &poll,
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();

    // The honest sibling staged; the offender staged nothing.
    let rows = usage_store.unapplied_events(100).unwrap();
    assert!(!rows.is_empty(), "the honest codex delta must stage");
    assert!(
        rows.iter().all(|row| row.event.provider_surface == "codex"),
        "no claude-code row may stage: {rows:?}"
    );
    assert_eq!(ids.len(), rows.len());
    let staged: f64 = rows.iter().map(|row| row.event.effective_tokens).sum();
    assert!((staged - 40_000.0).abs() < 0.01);

    // The offender's cursors advanced without rows (refused tokens are
    // never retro-fed)...
    assert_eq!(
        usage_store.latest_cursor_updated_at("claude-code").unwrap(),
        Some(now)
    );
    // ...with the diagnostic persisted for the offender only.
    let diagnostics = usage_store.recent_diagnostics(5).unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].provider_surface, "claude-code");
    assert_eq!(diagnostics[0].code, "usage_discontinuity");

    // The refusal narrates once and stamps the idle-narration cooldown so
    // the same pass cannot also narrate boredom.
    assert_eq!(
        state
            .recent_events
            .iter()
            .filter(|event| event.text == "mochi declined an implausible feast")
            .count(),
        1
    );
    assert_eq!(state.last_idle_narration_at, Some(now));

    // Complete the production sequence: the sibling's cursors advance on mark.
    let update = apply_unapplied_usage(&mut state, &mut usage_store, now, false).unwrap();
    usage_store
        .mark_events_applied_and_advance_cursors(&update.applied_event_ids, now)
        .unwrap();
    assert_eq!(
        usage_store.latest_cursor_updated_at("codex").unwrap(),
        Some(now)
    );
}

#[test]
fn multi_day_vacation_catchup_passes_the_guard_via_days_factor() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 20_000_000.0;
    let now = datetime!(2026 - 06 - 10 08:00 UTC);
    // The provider last fed 6 days ago: days_factor = 6 + 1 = 7.
    establish_provider_contact(&mut usage_store, "claude-code", now - Duration::days(6));

    // Six honest days arrive at once: 6 x 20M = 120M. A factor-1 threshold
    // (max(5 x 20M, 50M) = 100M) would refuse this; the per-provider
    // days_factor lifts it to 700M and the catch-up feeds.
    let poll = poll_with_delta(120_000_000.0, now);
    stage_usage_poll_deltas(
        &mut usage_store,
        &poll,
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();

    let rows = usage_store.unapplied_events(200).unwrap();
    let staged: f64 = rows.iter().map(|row| row.event.effective_tokens).sum();
    assert!((staged - 120_000_000.0).abs() < 0.01);
    assert!(usage_store.recent_diagnostics(5).unwrap().is_empty());
    assert!(state.recent_events.is_empty(), "no refusal narration");
    assert_eq!(state.last_idle_narration_at, None);
}

#[test]
fn first_contact_provider_is_refused_without_staging_history() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    let now = datetime!(2026 - 06 - 10 08:00 UTC);

    // No cursors at all for "claude-code": a helper absent at init must not
    // feed its entire history on first appearance — however small the delta
    // (the calibration never-feed-history rule).
    let poll = poll_with_delta(1_000.0, now);
    let ids = stage_usage_poll_deltas(
        &mut usage_store,
        &poll,
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();

    assert!(ids.is_empty());
    assert_eq!(usage_store.unapplied_events(10).unwrap().len(), 0);
    assert_eq!(
        usage_store.latest_cursor_updated_at("claude-code").unwrap(),
        Some(now)
    );
    let diagnostics = usage_store.recent_diagnostics(5).unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "usage_discontinuity");

    // The next poll is past first contact and feeds normally.
    let next = now + Duration::minutes(10);
    let staged_ids = stage_usage_poll_deltas(
        &mut usage_store,
        &poll_with_delta(1_000.0, next),
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        next,
    )
    .unwrap();
    assert!(!staged_ids.is_empty());
}

#[test]
fn guard_floor_passes_heavy_honest_days_over_a_low_median_baseline() {
    let dir = tempdir().unwrap();
    let mut usage_store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    // A light user: ratio x baseline x 1 day = 5 x 2M = 10M, far below the
    // 50M floor. A same-day 30M honest heavy catch-up must still feed.
    state.calibration.daily_effective_tokens = 2_000_000.0;
    let now = datetime!(2026 - 06 - 10 08:00 UTC);
    establish_provider_contact(&mut usage_store, "claude-code", now - Duration::hours(2));

    let poll = poll_with_delta(30_000_000.0, now);
    stage_usage_poll_deltas(
        &mut usage_store,
        &poll,
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();

    let staged: f64 = usage_store
        .unapplied_events(200)
        .unwrap()
        .iter()
        .map(|row| row.event.effective_tokens)
        .sum();
    assert!((staged - 30_000_000.0).abs() < 0.01);
    assert!(usage_store.recent_diagnostics(5).unwrap().is_empty());
}
```

And add a config test mod at the bottom of `src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discontinuity_guard_ratio_defaults_when_absent_from_config() {
        let config: AppConfig = toml::from_str("cache_read_weight = 0.05").unwrap();
        assert_eq!(
            config.discontinuity_guard_ratio,
            crate::game::runtime::DISCONTINUITY_GUARD_RATIO
        );
        assert_eq!(AppConfig::default().discontinuity_guard_ratio, 5.0);
    }

    #[test]
    fn discontinuity_guard_ratio_is_overridable() {
        let config: AppConfig = toml::from_str("discontinuity_guard_ratio = 12.5").unwrap();
        assert_eq!(config.discontinuity_guard_ratio, 12.5);
        assert_eq!(config.cache_read_weight, 0.03);
    }
}
```

- [ ] **Step 2: Run the tests, watch them fail**

Run: `cargo test --test runtime_integration discontinuity`
Expected: compile errors — `error[E0061]: this function takes 4 arguments but 5 arguments were supplied` on the new tests (and `error[E0425]`/unresolved `DISCONTINUITY_GUARD_RATIO`). `cargo test --lib config` fails on the missing field. This is the red state for a binding signature change.

- [ ] **Step 3: Implement the guard, the signature change, and the production threading**

**`src/config.rs`** — full new file content (it is 41 lines; the field, default fn, and Default impl change):

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_cache_read_weight")]
    pub cache_read_weight: f64,
    /// Multiplier on the per-provider discontinuity threshold
    /// (`guard_ratio x baseline x days_factor`, floored at 50M effective
    /// tokens). The escape hatch for users the guard refuses honestly.
    #[serde(default = "default_discontinuity_guard_ratio")]
    pub discontinuity_guard_ratio: f64,
}

fn default_cache_read_weight() -> f64 {
    0.03
}

fn default_discontinuity_guard_ratio() -> f64 {
    crate::game::runtime::DISCONTINUITY_GUARD_RATIO
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            cache_read_weight: default_cache_read_weight(),
            discontinuity_guard_ratio: default_discontinuity_guard_ratio(),
        }
    }
}

impl AppConfig {
    pub fn load_or_default(path: &Path) -> crate::error::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let text = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&text).map_err(|err| {
            crate::error::GlorpError::Message(format!("malformed config.toml: {err}"))
        })?;

        if !(0.0..=1.0).contains(&config.cache_read_weight) {
            return Err(crate::error::GlorpError::Message(
                "cache_read_weight must be between 0.0 and 1.0".into(),
            ));
        }

        Ok(config)
    }
}
```

(No range validation on the ratio: the `max(.., DISCONTINUITY_GUARD_FLOOR_TOKENS)` floor makes even a zero or negative ratio safe, and the ratio is the documented escape hatch.)

**`src/game/runtime.rs`** — add to the constants block (after `LIVE_SIGNAL_BACKFILL_DAILY_RATIO` at `src/game/runtime.rs:25`):

```rust
/// A provider surface whose summed poll delta exceeds
/// `guard_ratio x baseline x days_factor` (floored below) is refused:
/// cursors advance, nothing stages. Config-overridable via
/// `discontinuity_guard_ratio` (spec Amendment 2026-06-10).
pub const DISCONTINUITY_GUARD_RATIO: f64 = 5.0;
/// Guard threshold floor — a same-day honest heavy catch-up over a
/// low-median baseline must pass while a true bolus still fires.
pub const DISCONTINUITY_GUARD_FLOOR_TOKENS: f64 = 50_000_000.0;
/// Diagnostic code persisted on a refused poll. Exempt from source-health
/// broken classification and the ready-today filter (the source is
/// healthy — one poll was refused).
pub const USAGE_DISCONTINUITY_CODE: &str = "usage_discontinuity";
```

Extend the `storage::usage_store` import at `src/game/runtime.rs:14` to include `ProviderDiagnostic`:

```rust
        usage_store::{NormalizedUsageEvent, ProviderDiagnostic, UsageLedgerRow, UsageStore},
```

Replace the `stage_usage_poll_deltas` head (Read `src/game/runtime.rs:34-74` first; only the signature, the first lines, and a `continue` in the loop change — the smear body stays verbatim):

```rust
pub fn stage_usage_poll_deltas(
    usage_store: &mut UsageStore,
    poll: &UsagePollResult,
    state: &mut PetState,
    guard_ratio: f64,
    now: OffsetDateTime,
) -> Result<Vec<i64>> {
    let baseline = state.calibration;
    let refused_surfaces = refuse_discontinuous_surfaces(usage_store, poll, state, guard_ratio, now)?;
    let mut ids = Vec::new();
    let current_bucket = floor_to_ten_minute_bucket(now);
    for delta in &poll.deltas {
        if refused_surfaces.contains(&delta.provider_surface) {
            continue;
        }
        let buckets = crate::game::catchup::smear_catchup_delta(delta.effective_tokens, baseline);
```

(everything from `let bucket_count = buckets.len();` onward is unchanged.)

Add the guard helper right after `stage_usage_poll_deltas`:

```rust
/// The usage discontinuity guard (spec Amendment 2026-06-10). Per provider
/// surface, a poll whose summed effective delta exceeds
/// `max(guard_ratio x baseline x days_factor, DISCONTINUITY_GUARD_FLOOR_TOKENS)`
/// is refused alone: its cursors advance with a persisted
/// `usage_discontinuity` diagnostic in one transaction, and nothing stages.
/// `days_factor` = whole days since that provider's newest cursor
/// `updated_at` + 1 — per-provider, so an honest multi-day catch-up after a
/// single-helper outage is not pinned at factor 1 by its healthy sibling. A
/// surface with no cursors at all is first contact and is refused outright
/// (the calibration never-feed-history rule). Refusal narrates once and
/// stamps `last_idle_narration_at` so the same pass cannot also narrate
/// boredom. Refused tokens are never retro-fed; the config ratio is the
/// escape hatch going forward.
fn refuse_discontinuous_surfaces(
    usage_store: &mut UsageStore,
    poll: &UsagePollResult,
    state: &mut PetState,
    guard_ratio: f64,
    now: OffsetDateTime,
) -> Result<std::collections::BTreeSet<String>> {
    let mut surface_sums: std::collections::BTreeMap<String, f64> =
        std::collections::BTreeMap::new();
    for delta in &poll.deltas {
        *surface_sums
            .entry(delta.provider_surface.clone())
            .or_insert(0.0) += delta.effective_tokens.max(0.0);
    }

    let mut refused = std::collections::BTreeSet::new();
    for (surface, sum) in &surface_sums {
        let message = match usage_store.latest_cursor_updated_at(surface)? {
            None => format!(
                "first contact: refused {sum:.0} effective tokens (history never feeds)"
            ),
            Some(updated_at) => {
                let days_factor = ((now - updated_at).whole_days().max(0) + 1) as f64;
                let threshold = (guard_ratio
                    * state.calibration.daily_effective_tokens
                    * days_factor)
                    .max(DISCONTINUITY_GUARD_FLOOR_TOKENS);
                if *sum <= threshold {
                    continue;
                }
                format!("refused {sum:.0} effective tokens (threshold {threshold:.0})")
            }
        };
        let updates: Vec<_> = poll
            .deltas
            .iter()
            .filter(|delta| &delta.provider_surface == surface)
            .map(|delta| delta.cursor_update.clone())
            .collect();
        usage_store.refuse_poll_discontinuity(
            updates,
            &ProviderDiagnostic {
                provider_surface: surface.clone(),
                code: USAGE_DISCONTINUITY_CODE.to_string(),
                message,
                recorded_at: now,
            },
            now,
        )?;
        refused.insert(surface.clone());
    }

    if !refused.is_empty() {
        state.recent_events.push(NarrativeEvent {
            observed_at: now,
            text: format!("{} declined an implausible feast", state.pet.accepted_name),
        });
        state.last_idle_narration_at = Some(now);
    }
    Ok(refused)
}
```

(One narrative event per refused pass, not per surface — two surfaces refused on one poll would otherwise push two identical lines into the feed. The diagnostics stay per-surface. Cursor advance for a refused surface is exactly the `glorp init` calibration precedent at `src/commands/init.rs:40-46`: cursors move, no ledger rows, no food.)

Update the `#[doc(hidden)]` wrapper (Read `src/game/runtime.rs:348-358`; replace the stage line):

```rust
    stage_usage_poll_deltas(usage_store, poll, state, DISCONTINUITY_GUARD_RATIO, now)?;
```

**`src/commands/status.rs`** — Read `src/commands/status.rs:31-47` first; the stage line at :34 becomes:

```rust
                    stage_usage_poll_deltas(
                        &mut usage_store,
                        &result,
                        &mut state,
                        config.discontinuity_guard_ratio,
                        now,
                    )?;
```

(`config` is already in scope from `src/commands/status.rs:26`.)

**`src/commands/watch.rs`** — Read `src/commands/watch.rs:393-407` first; the stage line at :395 becomes:

```rust
        stage_usage_poll_deltas(
            &mut usage_store,
            &result,
            &mut state,
            config.discontinuity_guard_ratio,
            now,
        )?;
```

(`config` is already in scope from `src/commands/watch.rs:388`. This `poll_usage_and_apply` chokepoint also serves the menubar — `src/menubar/app.rs:178` — so both live frontends are guarded by this one edit.)

- [ ] **Step 4: Mechanically migrate every remaining caller**

Every test that stages deltas onto a fresh store now hits first-contact refusal, and every direct `stage_usage_poll_deltas` call needs the new signature. Read each region before editing.

**`src/commands/watch.rs` in-module tests** — add a seeding helper inside `mod tests` (after `sample_event_at_for_test` at `src/commands/watch.rs:761-767`; `ProviderCursorUpdate` is already imported at :755):

```rust
    fn establish_contact_for_test(usage: &mut UsageStore, surface: &str, now: OffsetDateTime) {
        usage
            .advance_cursors(
                vec![ProviderCursorUpdate {
                    provider_surface: surface.to_string(),
                    cursor_key: format!("{surface}-first-contact"),
                    cursor_value: "seeded".to_string(),
                    provider_version: "test-provider".to_string(),
                    parser_version: "test-parser".to_string(),
                }],
                now,
            )
            .unwrap();
    }
```

In `oversized_staged_backlog_becomes_visible_over_successive_applies` (Read `src/commands/watch.rs:1102-1140`): insert `establish_contact_for_test(&mut usage, "claude-code", now);` right after the `let mapper = ...` line, and change the loop body call

```rust
            stage_usage_poll_deltas(&mut usage, &poll, state.calibration, now).unwrap();
```

to

```rust
            stage_usage_poll_deltas(
                &mut usage,
                &poll,
                &mut state,
                crate::game::runtime::DISCONTINUITY_GUARD_RATIO,
                now,
            )
            .unwrap();
```

In `cold_start_catchup_wakes_the_pet_once_through_the_real_smear_path` (Read `src/commands/watch.rs:1142-1192`): insert `establish_contact_for_test(&mut usage, "claude-code", now);` right before the `// Drive the REAL smear` comment, and change

```rust
        stage_usage_poll_deltas(&mut usage, &poll, state.calibration, now).unwrap();
```

to

```rust
        stage_usage_poll_deltas(
            &mut usage,
            &poll,
            &mut state,
            crate::game::runtime::DISCONTINUITY_GUARD_RATIO,
            now,
        )
        .unwrap();
```

**`tests/runtime_integration.rs` existing tests** — seed contact before the first delta-carrying poll in each (the `empty_poll()`-only tests need nothing):

- `provider_delta_updates_pet_state_and_records_evolution_once` (:22): after `let now = datetime!(2026 - 05 - 09 12:00 UTC);` insert `establish_provider_contact(&mut usage_store, "claude-code", now);`
- `rapid_token_polls_do_not_narrate_every_feed` (:92): after `let start = ...;` insert `establish_provider_contact(&mut usage_store, "claude-code", start);`
- `staged_usage_apportions_token_buckets_across_smear_rows` (:169): this test passes a `CalibrationBaseline` literal today. Replace

```rust
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    let mut poll = poll_with_delta(12_000.0, now);
```

with

```rust
    let now = datetime!(2026 - 05 - 09 12:00 UTC);
    let mut state = PetState::new_for_test("mochi-7f3a", "mochi");
    state.calibration.daily_effective_tokens = 100_000.0;
    establish_provider_contact(&mut usage_store, "claude-code", now);
    let mut poll = poll_with_delta(12_000.0, now);
```

and replace the call (the `glorp::game::calibration::CalibrationBaseline { ... }` argument disappears):

```rust
    let ids = stage_usage_poll_deltas(
        &mut usage_store,
        &poll,
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();
```

- `cold_start_does_not_narrate_initial_mood` (:284): after `let now = ...;` insert `establish_provider_contact(&mut usage_store, "claude-code", now);`
- `lifetime_threshold_unlocks_one_ladder_prop_once` (:471), `one_large_poll_unlocks_ladder_props_in_threshold_order` (:498), `heavy_session_unlocks_planter_once` (:612), `wilted_recovery_unlocks_sprout_once` (:640): same one-line `establish_provider_contact(&mut usage_store, "claude-code", now);` after the `let now = ...;` line in each.
- `reflected_unapplied_usage_does_not_unlock_ladder_twice_on_mark_retry` (:544): insert `establish_provider_contact(&mut usage_store, "claude-code", now);` after `let now = ...;`, and replace

```rust
    stage_usage_poll_deltas(
        &mut usage_store,
        &poll_with_delta(60_000.0, now),
        state.calibration,
        now,
    )
    .unwrap();
```

with

```rust
    stage_usage_poll_deltas(
        &mut usage_store,
        &poll_with_delta(60_000.0, now),
        &mut state,
        DISCONTINUITY_GUARD_RATIO,
        now,
    )
    .unwrap();
```

- `first_codex_usage_unlocks_signal_lamp_once` (:580): after `let now = ...;` insert `establish_provider_contact(&mut usage_store, "codex", now);`

**`tests/usage_provider.rs`** — only `transcript_like_fields_are_ignored` (:194) asserts staged rows exist; the other `apply_usage_poll` users assert provider-level totals, which the guard does not touch (a refusal still advances cursors, which is all `complete_poll_lifecycle` exists to do). Change the import at :4 to

```rust
use glorp::storage::usage_store::{ProviderCursorUpdate, UsageStore};
```

and in `transcript_like_fields_are_ignored`, before the `apply_usage_poll` call at :203, insert:

```rust
    for surface in ["claude-code", "codex"] {
        store
            .advance_cursors(
                vec![ProviderCursorUpdate {
                    provider_surface: surface.to_string(),
                    cursor_key: format!("{surface}-first-contact"),
                    cursor_value: "seeded".to_string(),
                    provider_version: "test-provider".to_string(),
                    parser_version: "test-parser".to_string(),
                }],
                OffsetDateTime::now_utc(),
            )
            .unwrap();
    }
```

**`tests/doctor_status.rs`** — `status_persists_real_usage_delta_into_pet_state` (:200) writes a pet state without `glorp init`, so its providers are first contact. Add the import and helper at the top of the file:

```rust
use glorp::storage::usage_store::{ProviderCursorUpdate, UsageStore};
```

```rust
fn establish_provider_contact(dir: &std::path::Path, surfaces: &[&str]) {
    let mut usage = UsageStore::open(&dir.join("usage.sqlite")).unwrap();
    for surface in surfaces {
        usage
            .advance_cursors(
                vec![ProviderCursorUpdate {
                    provider_surface: surface.to_string(),
                    cursor_key: format!("{surface}-first-contact"),
                    cursor_value: "seeded".to_string(),
                    provider_version: "test-provider".to_string(),
                    parser_version: "test-parser".to_string(),
                }],
                time::OffsetDateTime::now_utc(),
            )
            .unwrap();
    }
}
```

and in `status_persists_real_usage_delta_into_pet_state`, after the `.save(&state).unwrap();` call, insert:

```rust
    establish_provider_contact(dir.path(), &["claude-code", "codex"]);
```

**`tests/game_rules.rs`** — the struct literal at :30 no longer compiles; change

```rust
    let config = AppConfig {
        cache_read_weight: 0.05,
    };
```

to

```rust
    let config = AppConfig {
        cache_read_weight: 0.05,
        ..AppConfig::default()
    };
```

- [ ] **Step 5: Run the full suite, verify green**

Run: `cargo test`
Expected: all test binaries pass, including the four new guard tests (`cargo test --test runtime_integration discontinuity` → 1 passed; `vacation` → 1 passed; `first_contact` → 1 passed; `guard_floor` → 1 passed) and the two config tests. Test output must be pristine.

- [ ] **Step 6: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add src/config.rs src/game/runtime.rs src/commands/status.rs src/commands/watch.rs tests/runtime_integration.rs tests/usage_provider.rs tests/doctor_status.rs tests/game_rules.rs
git commit -m "feat(runtime): refuse implausible per-provider usage discontinuities"
```

---

### Task 3: honest surfacing + local feed timestamps

**Spec sections:** Amendment item 1 surfacing rules ("the `usage_discontinuity` diagnostic code is exempt from source-health's broken classification and the ready-today filter") and Amendment item 2 ("Local feed timestamps... Display-only fix: format via the mapper's local offset. No stored data changes."). Interface sheet: `format_hhmm_local(now, offset)` lives where `format_hhmm` lives (`src/pet/activity.rs`); all EventView timestamp formatting goes through it; callers thread the offset — vm build: `mapper.offset_at(now)`, install paths: `LocalDayMapper::System`.

**Files:**
- Modify: `src/commands/watch.rs` (`source_health` at :481, `active_diagnostics` at :532, `build_recent_events` at :546, `aggregated_recent_usage_with_time` at :599, `deduped_recent_diagnostics` at :668, `timestamp_column` at :744, error-path EventView at :354, `derive_pet_activities` call at :125)
- Modify: `src/commands/status.rs` (store-read surfacing of the refusal without claiming blocked)
- Modify: `src/pet/activity.rs` (`format_hhmm_local`; offset params; existing in-module tests)
- Modify: `src/tui/app.rs` (string `timestamp_column` at :776 routes through `format_hhmm_local`; `append_profile_pet_activities` at :633)
- Test: in-module tests in `src/commands/watch.rs`, `src/pet/activity.rs`, `src/tui/app.rs`; integration test in `tests/doctor_status.rs`

This task is two red/green cycles (surfacing, then timestamps) with one commit.

- [ ] **Step 1: Write the failing surfacing tests**

Add to `mod tests` in `src/commands/watch.rs` (`SourceStatus`, `SourceUsageView`, and `OffsetDateTime` are in scope via `use super::*;`):

```rust
    fn discontinuity_diagnostic_for_test(
        surface: &str,
        recorded_at: OffsetDateTime,
    ) -> crate::storage::usage_store::ProviderDiagnostic {
        crate::storage::usage_store::ProviderDiagnostic {
            provider_surface: surface.to_string(),
            code: crate::game::runtime::USAGE_DISCONTINUITY_CODE.to_string(),
            message: "refused 212000000 effective tokens (threshold 99000000)".to_string(),
            recorded_at,
        }
    }

    #[test]
    fn usage_discontinuity_does_not_mark_a_source_broken() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let diagnostics = vec![discontinuity_diagnostic_for_test("claude-code", now)];

        // The surface fed earlier today, then one poll was refused: Ready,
        // with no diagnostic decoration — the source is healthy.
        let today = vec![("claude-code".to_string(), 12_000.0)];
        let health = source_health(&today, &[], &diagnostics);
        assert_eq!(health.len(), 1);
        assert_eq!(health[0].status, SourceStatus::Ready);
        assert_eq!(health[0].diagnostic_code, None);
        assert_eq!(health[0].diagnostic_message, None);

        // Even with zero tokens today, a refusal alone must not conjure a
        // blocked/diagnostic source row.
        let health = source_health(&[], &[], &diagnostics);
        assert!(
            health.is_empty(),
            "a discontinuity-only surface must not appear broken: {health:?}"
        );
    }

    #[test]
    fn usage_discontinuity_survives_the_ready_today_filter() {
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let helper_exit = crate::storage::usage_store::ProviderDiagnostic {
            provider_surface: "codex".to_string(),
            code: "helper_exit".to_string(),
            message: "helper exited 2".to_string(),
            recorded_at: now,
        };
        let sources = vec![
            SourceUsageView {
                name: "claude-code".to_string(),
                effective_tokens: 12_000.0,
            },
            SourceUsageView {
                name: "codex".to_string(),
                effective_tokens: 500.0,
            },
        ];

        let active = active_diagnostics(
            &sources,
            vec![
                discontinuity_diagnostic_for_test("claude-code", now),
                helper_exit,
            ],
        );

        // ready-today silences codex's stale helper_exit, but the refusal
        // stays visible even though claude-code fed today.
        assert_eq!(active.len(), 1);
        assert_eq!(
            active[0].code,
            crate::game::runtime::USAGE_DISCONTINUITY_CODE
        );
    }
```

Add to `tests/doctor_status.rs` (uses the Task 2 first-contact refusal as the real trigger — no fixtures are hand-built):

```rust
#[test]
fn status_surfaces_usage_discontinuity_without_claiming_blocked() {
    let dir = tempdir().unwrap();
    let mut state = PetState::new_for_test("fixture-seed", "mochi");
    state.calibration.daily_effective_tokens = 10_000.0;
    glorp::storage::state::StateStore::new(dir.path().join("state.json"))
        .save(&state)
        .unwrap();
    // Deliberately NO establish_provider_contact: both helpers are first
    // contact, so the guard refuses their history and persists the
    // usage_discontinuity diagnostic during this status run.

    Command::cargo_bin("glorp")
        .unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", "tests/fixtures/helpers/ccusage-ok.mjs")
        .env(
            "GLORP_CCUSAGE_CODEX_BIN",
            "tests/fixtures/helpers/ccusage-codex-ok.mjs",
        )
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("provider: local-log-derived"))
        .stdout(predicate::str::contains("provider health: ok"))
        .stdout(predicate::str::contains("diagnostic: usage_discontinuity"))
        .stdout(predicate::str::contains("declined an implausible feast"))
        .stdout(predicate::str::contains("blocked").not());

    let saved: PetState =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("state.json")).unwrap())
            .unwrap();
    assert_eq!(
        saved.lifetime_effective_tokens, 0.0,
        "refused tokens never feed"
    );
}
```

- [ ] **Step 2: Run the surfacing tests, watch them fail**

Run: `cargo test -p glorp --lib usage_discontinuity` and `cargo test --test doctor_status status_surfaces`
Expected: `usage_discontinuity_does_not_mark_a_source_broken` fails (`diagnostic_code` is `Some("usage_discontinuity")` and the no-tokens case yields a `Diagnostic`-status row); `usage_discontinuity_survives_the_ready_today_filter` fails (`active.len()` is 0 — both filtered); `status_surfaces_usage_discontinuity_without_claiming_blocked` fails on the missing `diagnostic: usage_discontinuity` stdout line.

- [ ] **Step 3: Implement the surfacing exemptions**

**`src/commands/watch.rs` `source_health`** (Read `src/commands/watch.rs:481-530` first). The names loop gains a skip:

```rust
    for diagnostic in diagnostics {
        if diagnostic.code == crate::game::runtime::USAGE_DISCONTINUITY_CODE {
            continue; // a refused poll is not a broken source
        }
        names.insert(diagnostic.provider_surface.clone());
    }
```

and the per-name diagnostic lookup excludes the code:

```rust
            let diagnostic = diagnostics.iter().find(|diagnostic| {
                diagnostic.provider_surface == name
                    && diagnostic.code != crate::game::runtime::USAGE_DISCONTINUITY_CODE
            });
```

**`src/commands/watch.rs` `active_diagnostics`** (Read `src/commands/watch.rs:532-544` first). The filter becomes:

```rust
    diagnostics
        .into_iter()
        .filter(|diagnostic| {
            // The refusal is real information independent of source health:
            // it survives the ready-today filter (exemption, spec Amendment).
            diagnostic.code == crate::game::runtime::USAGE_DISCONTINUITY_CODE
                || !ready_today.contains(diagnostic.provider_surface.as_str())
        })
        .collect()
```

**`src/commands/status.rs`** (Read `src/commands/status.rs:50-71` first). The guard's diagnostic is store-persisted during staging and never rides on `result.diagnostics`, so `provider_line` already stays "provider: local-log-derived"; status must additionally read it back. Insert after the existing `if let Some(diagnostic) = result.diagnostics.first() { ... } else { ... }` block (still inside the `Ok(result)` arm, where `usage_store` is in scope):

```rust
                // A refused poll persists its diagnostic in the store (the
                // guard writes it during staging); it never rides on
                // result.diagnostics. Surface it without claiming blocked —
                // the source is healthy, one poll was refused. Same 1h
                // freshness rule as the watch vm's stale-diagnostic cutoff.
                if diagnostic_line.is_none() {
                    let fresh_cutoff = OffsetDateTime::now_utc() - time::Duration::hours(1);
                    if let Ok(stored) = usage_store.recent_diagnostics(5) {
                        if let Some(refusal) = stored.iter().find(|diagnostic| {
                            diagnostic.code == crate::game::runtime::USAGE_DISCONTINUITY_CODE
                                && diagnostic.recorded_at >= fresh_cutoff
                        }) {
                            diagnostic_line = Some(format!(
                                "diagnostic: {} ({})",
                                refusal.code, refusal.provider_surface
                            ));
                        }
                    }
                }
```

- [ ] **Step 4: Run the surfacing tests, verify green**

Run: `cargo test -p glorp --lib usage_discontinuity` then `cargo test --test doctor_status`
Expected: 2 passed, then the full doctor_status binary green (including the new test and the Task 2-migrated `status_persists_real_usage_delta_into_pet_state`).

- [ ] **Step 5: Write the failing local-timestamp tests**

`src/pet/activity.rs` tests mod (these reference the new signature, so this cycle's red is a compile error):

```rust
    #[test]
    fn format_hhmm_local_renders_the_offset_clock_not_utc() {
        let now = datetime!(2026-06-09 06:00 UTC); // 23:00 the previous evening at UTC-7
        let offset = time::UtcOffset::from_hms(-7, 0, 0).unwrap();
        assert_eq!(format_hhmm_local(now, offset), "23:00");
        assert_eq!(format_hhmm_local(now, time::UtcOffset::UTC), "06:00");
    }

    #[test]
    fn activity_timestamps_thread_the_local_offset() {
        let now = datetime!(2026-06-09 03:10 UTC);
        let offset = time::UtcOffset::from_hms(-8, 0, 0).unwrap(); // 19:10 local
        let acts = derive_pet_activities(
            "vex",
            Species::Mech,
            Mood::Happy,
            &[],
            &[],
            now,
            offset,
        );
        assert!(!acts.is_empty());
        assert!(
            acts.iter().all(|e| e.timestamp == "19:10"),
            "expected local 19:10 stamps: {acts:?}"
        );
    }
```

`src/commands/watch.rs` tests mod — end-to-end through the vm build (the Amendment's incident shape: last night's 23:00 PDT feed must not display as 06:00):

```rust
    #[test]
    fn feed_timestamps_render_the_mapper_local_clock_not_utc() {
        use crate::storage::state::NarrativeEvent;
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        UsageStore::open(&db_path).unwrap();
        let now = PrimitiveDateTime::new(
            Date::from_calendar_date(2026, Month::June, 10).unwrap(),
            Time::from_hms(6, 30, 0).unwrap(),
        )
        .assume_utc();
        let mut state = PetState::new_for_test("test", "buddy");
        state.recent_events = vec![NarrativeEvent {
            observed_at: now - Duration::minutes(30), // 06:00 UTC = 23:00 at UTC-7
            text: "buddy munched 1.0k tokens".into(),
        }];
        let mapper = LocalDayMapper::Fixed(time::UtcOffset::from_hms(-7, 0, 0).unwrap());

        let vm = build_watch_view_model_at(&state, &db_path, now, mapper).unwrap();

        let feed = vm
            .recent_events
            .iter()
            .find(|event| event.text.contains("munched"))
            .unwrap();
        assert_eq!(
            feed.timestamp, "23:00",
            "last night's 23:00 local feed must not display as 06:00 UTC"
        );
    }
```

`src/tui/app.rs` tests mod (the harness path keeps the string's own offset so test frames stay machine-independent, but it must route through the one formatter):

```rust
    #[test]
    fn harness_timestamp_column_formats_hhmm_in_the_string_offset() {
        assert_eq!(super::timestamp_column("2026-05-09T13:42:00Z"), "13:42");
        assert_eq!(
            super::timestamp_column("2026-05-09T23:00:00-07:00"),
            "23:00"
        );
        assert_eq!(super::timestamp_column("not a timestamp"), "--:--");
    }
```

- [ ] **Step 6: Run the timestamp tests, watch them fail**

Run: `cargo test -p glorp --lib format_hhmm_local`
Expected: compile error — `error[E0425]: cannot find function 'format_hhmm_local'` and `error[E0061]` on the 7-argument `derive_pet_activities` call. (`feed_timestamps_render_the_mapper_local_clock_not_utc` compiles but fails red with `06:00 != 23:00` once the others are stubbed; observe at least the compile-error red before implementing.)

- [ ] **Step 7: Implement `format_hhmm_local` and thread the offset everywhere**

**`src/pet/activity.rs`** — replace the private `format_hhmm` (`src/pet/activity.rs:105-107`) with the public binding helper:

```rust
/// Format an instant as a local-clock `hh:mm` label. All EventView timestamp
/// formatting goes through this; callers thread the offset (vm build:
/// `mapper.offset_at(now)`; install paths: `LocalDayMapper::System`).
pub fn format_hhmm_local(now: OffsetDateTime, offset: time::UtcOffset) -> String {
    let local = now.to_offset(offset);
    format!("{:02}:{:02}", local.hour(), local.minute())
}
```

`derive_pet_activities` (`src/pet/activity.rs:32-39`) gains a trailing param and every internal `format_hhmm(now)` becomes `format_hhmm_local(now, local_offset)`:

```rust
pub fn derive_pet_activities(
    pet_name: &str,
    species: Species,
    mood: Mood,
    usage_events: &[NormalizedUsageEvent],
    seen_stage_transitions: &[Stage],
    now: OffsetDateTime,
    local_offset: time::UtcOffset,
) -> Vec<EventView> {
```

`derive_profile_pet_activities` (`src/pet/activity.rs:78-84`) gains the same trailing `local_offset: time::UtcOffset` param; its `format_hhmm(now)` at :99 becomes `format_hhmm_local(now, local_offset)`.

Mechanically update the existing in-module tests (8 call sites at `src/pet/activity.rs:211-212, 220, 227, 235, 248, 260, 268, 289`): append `time::UtcOffset::UTC` as the final argument to every `derive_pet_activities` / `derive_profile_pet_activities` call.

**`src/commands/watch.rs`** — delete `fn timestamp_column` (`src/commands/watch.rs:744-746`) and thread the offset:

- `build_recent_events` (Read `src/commands/watch.rs:546-593` first) gains a trailing `local_offset: time::UtcOffset` param; the narrative branch's `timestamp_column(event.observed_at)` becomes `crate::pet::activity::format_hhmm_local(event.observed_at, local_offset)`; its inner calls become `aggregated_recent_usage_with_time(usage_events, 4, local_offset)` and `deduped_recent_diagnostics(diagnostics, 2, local_offset)`.
- `aggregated_recent_usage_with_time` (:599) gains trailing `local_offset: time::UtcOffset`; `timestamp_column(observed_at)` at :653 becomes `crate::pet::activity::format_hhmm_local(observed_at, local_offset)`.
- `deduped_recent_diagnostics` (:668) gains trailing `local_offset: time::UtcOffset`; `timestamp_column(diagnostic.recorded_at)` at :686 becomes `crate::pet::activity::format_hhmm_local(diagnostic.recorded_at, local_offset)`.
- The vm build call sites (Read `src/commands/watch.rs:125-133`; `local_offset` is already in scope from `src/commands/watch.rs:90`):

```rust
    let pet_activities = crate::pet::activity::derive_pet_activities(
        &state.pet.accepted_name,
        species,
        mood,
        &recent_usage,
        &state.seen_stage_transitions,
        now,
        local_offset,
    );
    let recent_events =
        build_recent_events(state, &recent_usage, &diagnostics, pet_activities, local_offset);
```

- The poller error path (Read `src/commands/watch.rs:349-365`; this is an install path → `LocalDayMapper::System`):

```rust
            Err(err) => {
                let mut vm = current.clone();
                vm.helper_status = "provider poll failed".into();
                vm.errors.push(err.to_string());
                let now = OffsetDateTime::now_utc();
                vm.recent_events.push(EventView {
                    timestamp: crate::pet::activity::format_hhmm_local(
                        now,
                        LocalDayMapper::System.offset_at(now),
                    ),
                    kind: LogKind::Diagnostic,
                    text: err.to_string(),
                });
```

(the rest of the arm is unchanged.)

- The in-module test `build_recent_events_interleaves_narrative_and_usage_by_timestamp` (:997): the call becomes `build_recent_events(&state, &usage_events, &[], vec![], time::UtcOffset::UTC);`

**`src/tui/app.rs`** — replace `timestamp_column` (`src/tui/app.rs:776-783`):

```rust
fn timestamp_column(timestamp: &str) -> String {
    // Test-harness path: render in the offset the string itself carries so
    // harness frames stay machine-independent, routed through the one
    // EventView formatter.
    time::OffsetDateTime::parse(timestamp, &time::format_description::well_known::Rfc3339)
        .map(|instant| crate::pet::activity::format_hhmm_local(instant, instant.offset()))
        .unwrap_or_else(|_| "--:--".into())
}
```

and `append_profile_pet_activities` (Read `src/tui/app.rs:633-650`; install path → System):

```rust
fn append_profile_pet_activities(vm: &mut WatchViewModel, now: time::OffsetDateTime) {
    let local_offset = crate::storage::day_axis::LocalDayMapper::System.offset_at(now);
    let activities = crate::pet::activity::derive_profile_pet_activities(
        &vm.pet_name,
        vm.pet_render.generated_species,
        vm.pet_render.mood,
        &vm.life_profile,
        now,
        local_offset,
    );
```

(the dedup loop below is unchanged.)

- [ ] **Step 8: Run the full suite, verify green**

Run: `cargo test`
Expected: everything green — including the three new timestamp tests, the untouched dev_preview snapshots (Preview Lab pins `LocalDayMapper::Fixed(UtcOffset::UTC)`, so frame output is byte-identical), and `tests/watch_integration.rs` / `tests/tui_render.rs` (harness strings carry `Z`, so their hh:mm output is unchanged). Run `cargo test --test dev_preview` explicitly to confirm zero snapshot churn.

- [ ] **Step 9: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add src/commands/watch.rs src/commands/status.rs src/pet/activity.rs src/tui/app.rs tests/doctor_status.rs
git commit -m "feat(watch): surface poll refusals honestly and render local feed timestamps"
```
<!-- Part B: Tasks 4-6 — the creature: tiredness + voice -->

### Task 4: DayContext tiredness + `local_day_started_utc` + the morning-after window

**Spec sections:** Branch T2 — "Evening tiredness (its own vocabulary, not droop)" and "Morning-after" (window definition); "Honesty and degradation rules" (maturity gate governs every baseline-ratio channel). Interface sheet: "DayContext additions (src/tui/day.rs)" + constants `FATIGUE_WINDOW_HOURS=16`, `MORNING_AFTER_DAY_MINUTES=60`.

**Files:**
- Modify: `src/tui/day.rs` (constants at module top ~line 35; `DayContext` struct at 179-200; `Default` at 202-224; derivation inside `build_day_context` after the climate block at ~312-326; struct literal at 353-373; new pure helper after `scene_asleep_for_poll`)
- Test: in-module `#[cfg(test)] mod tests` at `src/tui/day.rs:535+` (reuses the shipped `utc_mapper`/`store_with_applied` helpers, day.rs:542-558)

**Anchor warning:** Read `src/tui/day.rs:179-224` (struct + Default) and `:287-373` (today/yesterday/climate block through the returned literal) before editing — the extraction contract pins these regions post-T1; copy `old_string` anchors from the live file.

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` in `src/tui/day.rs` (after `derive_wake_resume_pins_wake_resume_values`):

```rust
    /// Rows that satisfy the maturity gate (5 distinct active days, 3 distinct
    /// hours) while staying outside the trailing FATIGUE_WINDOW_HOURS at `now`
    /// AND outside today/yesterday: 2..=6 local days back.
    fn maturity_rows(now: time::OffsetDateTime) -> Vec<(time::OffsetDateTime, f64)> {
        let mut rows = Vec::new();
        for back in 2..=6_i64 {
            for hour in [9_i64, 13, 17] {
                rows.push((
                    now - time::Duration::days(back) - time::Duration::hours(hour - 12),
                    10_000.0,
                ));
            }
        }
        rows
    }

    #[test]
    fn tiredness_counts_active_buckets_not_elapsed_span() {
        // Lid-closed scenario (spec: "a heavy morning followed by a six-hour
        // lid-closed rest must not render near-max tiredness at 4pm").
        // Heavy morning: 24 ten-minute buckets (4 active hours) 06:00-10:00,
        // then nothing. Buckets are 10-minute-aligned, mirroring the
        // production smear's floored bucket_at values.
        let now = datetime!(2026-06-09 16:00 UTC);
        let mut rows = maturity_rows(now);
        let morning_start = datetime!(2026-06-09 06:00 UTC);
        for i in 0..24_i64 {
            rows.push((morning_start + time::Duration::minutes(i * 10), 20_000.0));
        }
        let store = store_with_applied(&rows);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(10);

        let at_four_pm = build_day_context(&store, &state, now, utc_mapper());
        assert!(at_four_pm.mature, "fixture must pass the maturity gate");
        // 24 active buckets / 96 window buckets = 0.25; the 480k window volume
        // saturates the volume ratio at 1.0 vs the 100k default baseline.
        assert!(
            (at_four_pm.tiredness - 0.25).abs() < 1e-6,
            "got {}",
            at_four_pm.tiredness
        );
        assert!(
            at_four_pm.tiredness < 0.5,
            "a rested afternoon must never read near-max"
        );

        // Same ledger evaluated right as the morning ended: identical
        // tiredness — six hours of rest added nothing, elapsed span is
        // irrelevant, only accumulated active time counts.
        let at_ten_am =
            build_day_context(&store, &state, datetime!(2026-06-09 10:00 UTC), utc_mapper());
        assert!((at_ten_am.tiredness - at_four_pm.tiredness).abs() < 1e-6);
    }

    #[test]
    fn light_days_stay_below_the_tired_motion_threshold() {
        // 3 small snack buckets (6k tokens vs the 100k baseline): the volume
        // term must keep tiredness below the 0.05 tired-motion gate.
        let now = datetime!(2026-06-09 12:00 UTC);
        let mut rows = maturity_rows(now);
        for i in 0..3_i64 {
            rows.push((
                now - time::Duration::hours(2) + time::Duration::minutes(i * 10),
                2_000.0,
            ));
        }
        let store = store_with_applied(&rows);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(10);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert!(ctx.mature);
        assert!(ctx.tiredness > 0.0, "real activity must register");
        assert!(
            ctx.tiredness < 0.05,
            "light days must not read tired, got {}",
            ctx.tiredness
        );
    }

    #[test]
    fn tiredness_is_zero_while_the_maturity_gate_is_closed() {
        // Heavy recent activity but only one active day: immature, and the
        // default 100k baseline can be 10-100x off in week one — no fatigue.
        let now = datetime!(2026-06-09 16:00 UTC);
        let mut rows = Vec::new();
        for i in 0..24_i64 {
            rows.push((
                now - time::Duration::hours(5) + time::Duration::minutes(i * 10),
                20_000.0,
            ));
        }
        let store = store_with_applied(&rows);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(10);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert!(!ctx.mature);
        assert_eq!(ctx.tiredness, 0.0, "immature pets never read tired");
    }

    #[test]
    fn local_day_started_utc_is_the_current_local_midnight() {
        let now = datetime!(2026-06-09 16:00 UTC);
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(2);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert_eq!(ctx.local_day_started_utc, datetime!(2026-06-09 00:00 UTC));
        assert_eq!(ctx.local_day_rollover_utc, datetime!(2026-06-10 00:00 UTC));
        // A non-UTC mapper anchors at that zone's midnight (same instant
        // comparison — OffsetDateTime PartialEq is offset-agnostic).
        let minus8 = LocalDayMapper::Fixed(time::UtcOffset::from_hms(-8, 0, 0).unwrap());
        let ctx_pst = build_day_context(&store, &state, now, minus8);
        assert_eq!(
            ctx_pst.local_day_started_utc,
            datetime!(2026-06-09 08:00 UTC),
            "local midnight at UTC-8 is 08:00 UTC"
        );
    }

    #[test]
    fn morning_after_window_covers_dawn_plus_first_day_hour_and_is_restart_idempotent() {
        let store = UsageStore::open(":memory:".as_ref()).unwrap();
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = datetime!(2026-06-07 00:00 UTC);
        // Empty ledger => immature => clock defaults: dawn 07-09, day 09-18.
        let cases = [
            (datetime!(2026-06-09 07:30 UTC), true),  // mid-Dawn
            (datetime!(2026-06-09 08:59 UTC), true),  // last Dawn minute
            (datetime!(2026-06-09 09:30 UTC), true),  // 30 min into Day
            (datetime!(2026-06-09 10:30 UTC), false), // 90 min into Day
            (datetime!(2026-06-09 20:00 UTC), false), // Dusk
            (datetime!(2026-06-09 02:00 UTC), false), // Night
        ];
        for (now, expected) in cases {
            let ctx = build_day_context(&store, &state, now, utc_mapper());
            assert_eq!(in_morning_after_window(&ctx, now), expected, "at {now}");
            // Restart idempotence: a freshly rebuilt context (new process,
            // same ledger) must agree — pure function of carried instants.
            let rebuilt = build_day_context(&store, &state, now, utc_mapper());
            assert_eq!(
                in_morning_after_window(&rebuilt, now),
                in_morning_after_window(&ctx, now)
            );
        }
    }
```

- [ ] **Step 2: Run tests, verify they fail to compile**

Run: `cargo test --lib tui::day 2>&1 | head -30`
Expected: compile errors — `E0609: no field "tiredness" on type DayContext`, `E0609: no field "local_day_started_utc"`, `E0425: cannot find function "in_morning_after_window"`.

- [ ] **Step 3: Implement**

Add the two constants after `PHASE_BLEND_MINUTES` (day.rs:35):

```rust
/// Trailing window for accumulated-active-time fatigue, in hours.
pub const FATIGUE_WINDOW_HOURS: i64 = 16;
/// Morning-after flavor covers all of Dawn plus this many minutes of Day.
pub const MORNING_AFTER_DAY_MINUTES: i64 = 60;
```

Append the two fields to `DayContext` (day.rs:179-200), after `local_day_rollover_utc` — doc comments verbatim from the interface sheet:

```rust
    /// Accumulated-active-time fatigue, 0.0..=1.0. Derived from the count of
    /// 10-minute buckets containing applied tokens in the trailing
    /// FATIGUE_WINDOW_HOURS, scaled by the window's volume ratio vs baseline.
    /// 0.0 while the maturity gate is closed.
    pub tiredness: f32,
    /// UTC instant the current local day began (motes tidy-fade anchor).
    pub local_day_started_utc: time::OffsetDateTime,
```

Extend `Default for DayContext` (day.rs:202-224) — add to the literal:

```rust
            tiredness: 0.0,
            local_day_started_utc: epoch,
```

In `build_day_context`, insert the derivation after the `climate` block (the `let climate = {...};` ending at day.rs:326) and before the sleep predicate. `baseline` and `today_start` are already in scope (day.rs:288-290):

```rust
    // --- tiredness: accumulated active time, not elapsed span ---
    // Count of active 10-minute buckets in the trailing window, scaled by the
    // window's volume ratio vs baseline. A heavy morning then a lid-closed
    // afternoon stays low; light days zero out via the volume term. Trailing
    // window => nothing snaps at midnight. Maturity-gated (spec: every
    // baseline-ratio channel).
    let tiredness = if mature {
        let fatigue_start = now - Duration::hours(FATIGUE_WINDOW_HOURS);
        let fatigue_buckets = usage_store
            .applied_bucket_sums_between(fatigue_start, now + Duration::seconds(1))
            .unwrap_or_default();
        let active_buckets = fatigue_buckets.iter().filter(|&&(_, t)| t > 0.0).count();
        let window_volume: f64 = fatigue_buckets.iter().map(|&(_, t)| t).sum();
        let active_share = active_buckets as f32 / (FATIGUE_WINDOW_HOURS * 6) as f32;
        let volume_ratio = ((window_volume / baseline) as f32).clamp(0.0, 1.0);
        (active_share * volume_ratio).clamp(0.0, 1.0)
    } else {
        0.0
    };
```

Add the two fields to the returned `DayContext` literal (day.rs:353-373), after `local_day_rollover_utc: tomorrow_start,`:

```rust
        tiredness,
        local_day_started_utc: today_start,
```

Add the pure helper after `scene_asleep_for_poll` (day.rs:376-383) — signature verbatim from the interface sheet:

```rust
/// Morning-after selection window: all of Dawn plus the first
/// MORNING_AFTER_DAY_MINUTES of Day. Pure function of carried instants.
pub fn in_morning_after_window(day: &DayContext, now: time::OffsetDateTime) -> bool {
    match day.day_phase {
        DayPhase::Dawn => true,
        DayPhase::Day => {
            now < day.phase_started_at_utc + Duration::minutes(MORNING_AFTER_DAY_MINUTES)
        }
        DayPhase::Dusk | DayPhase::Night => false,
    }
}
```

No other construction site breaks: the dev_preview fixtures build `DayContext` via `..DayContext::default()` struct-update (src/dev_preview/watch.rs:591-634) and pick up the new defaults.

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test --lib tui::day -- --nocapture`
Expected: 0 failed; the 5 new tests (`tiredness_counts_active_buckets_not_elapsed_span`, `light_days_stay_below_the_tired_motion_threshold`, `tiredness_is_zero_while_the_maturity_gate_is_closed`, `local_day_started_utc_is_the_current_local_midnight`, `morning_after_window_covers_dawn_plus_first_day_hour_and_is_restart_idempotent`) pass alongside the existing T1 day tests.

Run: `cargo test 2>&1 | tail -5`
Expected: full suite green (the fields are additive; Default covers every fixture).

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add src/tui/day.rs
git commit -m "feat(tui): derive tiredness and the morning-after window on DayContext"
```

---

### Task 5: the full speech precedence stack in `current_pet_speech_for_scene`

**Spec sections:** Branch T2 — "Dreams", "Morning-after" (authoring guardrail), "Speech precedence stack (binding, top wins)"; Honesty rules ("Dreams render ONLY when yesterday has dominant_shape detail", "Morning lines never reference the user's absence", "Needy vitals outrank every flavor channel"). Interface sheet: binding `current_pet_speech_for_scene` signature (REPLACES the T1 bool-asleep variant), `DREAM_WINDOW_MINUTES=10` in src/pet/speech.rs, wiring rule "install_poll_result and the vm build call current_pet_speech_for_scene(mood, profile, &day_context, now)".

**Files:**
- Modify: `src/pet/speech.rs` (replace `current_pet_speech_for_profile` at 47-62 and `current_pet_speech_for_scene` at 72-90; new dream/morning constants + helpers; update the three existing tests that call the old shapes)
- Modify: `src/tui/app.rs` (install_poll_result speech call at 569-574)
- Modify: `src/commands/watch.rs` (vm-build speech at 175-188; remove the superseded raw-token munch path: `RECENT_ACTIVITY_WINDOW` + `recent_activity_tokens` at 272-290 and its test at ~934-960)
- Test: in-module tests in `speech.rs`; one vm-level caller test in `watch.rs` tests

**Anchor warnings:** Read `src/pet/speech.rs:44-90` (the contract excerpt at speech.rs:64-90 names this branch as the dream splice point), `src/tui/app.rs:551-578` (install_poll_result), and `src/commands/watch.rs:175-196` + `:272-290` + `:925-960` before editing. **Supersession, stated explicitly (not silently):** the vm-build's legacy raw-token munch path (`current_pet_speech` call, `recent_activity_tokens`, `RECENT_ACTIVITY_WINDOW`, and the test `recent_activity_tokens_uses_bucket_at_not_observed_at`) is replaced by the profile-driven precedence stack mandated by the interface sheet; the removal is named in the commit message. `pub fn current_pet_speech` itself stays (still unit-tested; removing a pub API is out of scope for this task).

- [ ] **Step 1: Write the behaviorally-failing vm caller test (compiles against old code, fails red)**

Append to `mod tests` in `src/commands/watch.rs`:

```rust
    #[test]
    fn vm_build_speech_uses_the_scene_precedence_stack_not_raw_token_munch() {
        use time::macros::datetime;
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut store = UsageStore::open(&db_path).unwrap();
        // 1.8M effective tokens 5 minutes ago: the legacy raw-token path
        // munches on this; the profile-driven stack (default profile at vm
        // build — the live profile is re-stamped at install) must not.
        let now = datetime!(2026-05-11 12:00 UTC); // unix_ts % 30 == 0: visible slot
        store
            .insert_event(&sample_event_at_for_test(
                now - Duration::minutes(5),
                1_800_000.0,
            ))
            .unwrap();
        drop(store);
        let mut state = PetState::new_for_test("test", "buddy");
        state.created_at = now - Duration::days(3);

        let vm = build_watch_view_model_at(
            &state,
            &db_path,
            now,
            LocalDayMapper::Fixed(time::UtcOffset::UTC),
        )
        .unwrap();
        let line = vm.current_speech.expect("visible slot must produce a line");
        let munch = ["yum!", "more!", "tasty!", "delicious", "*chomp*"];
        assert!(
            !munch.contains(&line.as_str()),
            "vm build must not munch on raw recent tokens, got {line}"
        );
    }
```

- [ ] **Step 2: Run it, verify it fails for the right reason**

Run: `cargo test --lib vm_build_speech_uses_the_scene_precedence_stack -- --nocapture`
Expected: FAILED with `vm build must not munch on raw recent tokens, got yum!` (or another munch phrase) — the legacy `current_pet_speech(mood, recent_activity_tokens(...), now)` path fires on the 1.8M-token window.

- [ ] **Step 3: Write the failing speech unit tests (compile-red: new signature)**

In `src/pet/speech.rs` tests, REPLACE the bodies of `speech_uses_profile_burst_for_munch_reaction` (speech.rs:172-184), `speech_does_not_fake_munch_when_profile_is_idle` (:186-195), and `asleep_speech_is_a_sparse_zzz_cadence_and_suppresses_munch_and_mood_lines` (:218-243) and ADD three new tests:

```rust
    #[test]
    fn speech_uses_profile_burst_for_munch_reaction() {
        let visible = datetime!(2026-05-11 12:00 UTC);
        let profile = crate::tui::life::PetLifeProfile {
            burst_level: 1.0,
            ..Default::default()
        };

        let speech = current_pet_speech_for_scene(
            Mood::Content,
            &profile,
            &crate::tui::day::DayContext::default(),
            visible,
        )
        .unwrap();

        let munch_phrases = ["yum!", "more!", "tasty!", "delicious", "*chomp*"];
        assert!(munch_phrases.contains(&speech.as_str()));
    }

    #[test]
    fn speech_does_not_fake_munch_when_profile_is_idle() {
        let visible = datetime!(2026-05-11 12:00 UTC);
        let profile = crate::tui::life::PetLifeProfile::default();

        let speech = current_pet_speech_for_scene(
            Mood::Content,
            &profile,
            &crate::tui::day::DayContext::default(),
            visible,
        )
        .unwrap();

        let munch_phrases = ["yum!", "more!", "tasty!", "delicious", "*chomp*"];
        assert!(!munch_phrases.contains(&speech.as_str()));
    }

    #[test]
    fn asleep_speech_is_a_sparse_zzz_cadence_and_suppresses_munch_and_mood_lines() {
        use crate::tui::day::DayContext;
        // Visible slot of every SLEEP_SPEECH_CYCLE_N-th 30s cycle only.
        let cycle0 = OffsetDateTime::from_unix_timestamp(90 * (1_700_000_000 / 90)).unwrap();
        let hot_profile = crate::tui::life::PetLifeProfile {
            burst_level: 1.0, // would be a munch line awake
            ..Default::default()
        };
        let asleep = DayContext {
            asleep: true,
            ..Default::default()
        };
        let line = current_pet_speech_for_scene(Mood::Hungry, &hot_profile, &asleep, cycle0);
        assert!(
            matches!(line.as_deref(), Some(l) if SLEEP_SPEECH_PHRASES.contains(&l)),
            "asleep at an eligible cycle: zzz, never munch or 'feed me?' — got {line:?}"
        );
        // The next cycle (not a multiple of SLEEP_SPEECH_CYCLE_N) is silent.
        let cycle1 = cycle0 + time::Duration::seconds(30);
        assert_eq!(
            current_pet_speech_for_scene(Mood::Hungry, &hot_profile, &asleep, cycle1),
            None
        );
        // Awake, the live burst outranks even a needy mood (stack 3 > 4).
        let awake = current_pet_speech_for_scene(
            Mood::Hungry,
            &hot_profile,
            &DayContext::default(),
            cycle0,
        );
        let munch = ["yum!", "more!", "tasty!", "delicious", "*chomp*"];
        assert!(
            matches!(awake.as_deref(), Some(l) if munch.contains(&l)),
            "got {awake:?}"
        );
    }

    fn dawn_day(
        yesterday: Option<crate::tui::day::DaySummary>,
        mature: bool,
    ) -> crate::tui::day::DayContext {
        crate::tui::day::DayContext {
            day_phase: crate::tui::day::DayPhase::Dawn,
            mature,
            yesterday,
            ..Default::default()
        }
    }

    #[test]
    fn hungry_at_dawn_after_an_idle_yesterday_shows_the_vitals_line_not_a_greeting() {
        use crate::tui::day::DaySummary;
        // Two channels both keyed to the user working less must never stack
        // into nagging: the sanctioned vitals signal wins.
        let visible = datetime!(2026-05-11 12:00 UTC);
        let day = dawn_day(
            Some(DaySummary {
                ratio: 0.0,
                dominant_shape: None,
            }),
            true,
        );
        let line = current_pet_speech_for_scene(
            Mood::Hungry,
            &crate::tui::life::PetLifeProfile::default(),
            &day,
            visible,
        )
        .unwrap();
        let hungry = ["feed me?", "tokens?", "hungry...", "where's the food"];
        assert!(
            hungry.contains(&line.as_str()),
            "needy vitals outrank morning flavor, got {line}"
        );
    }

    #[test]
    fn morning_flavor_fires_for_observed_idle_yesterday_but_not_missing_coverage() {
        use crate::tui::day::DaySummary;
        let visible = datetime!(2026-05-11 12:00 UTC);
        let profile = crate::tui::life::PetLifeProfile::default();
        let content = ["hmm", "thinking deeply", "just chilling", "all is well"];

        // Some(0.0): an observed idle day selects the rested flavor.
        let observed_idle = dawn_day(
            Some(DaySummary {
                ratio: 0.0,
                dominant_shape: None,
            }),
            true,
        );
        let line =
            current_pet_speech_for_scene(Mood::Content, &profile, &observed_idle, visible).unwrap();
        assert!(
            MORNING_RESTED_PHRASES.contains(&line.as_str()),
            "Some(0.0) selects the rested flavor, got {line}"
        );

        // A feast yesterday reads mellow — the pet's own state, never the
        // user's absence (authoring guardrail).
        let feast = dawn_day(
            Some(DaySummary {
                ratio: 2.0,
                dominant_shape: None,
            }),
            true,
        );
        let line = current_pet_speech_for_scene(Mood::Content, &profile, &feast, visible).unwrap();
        assert!(
            MORNING_MELLOW_PHRASES.contains(&line.as_str()),
            "a feast yesterday reads mellow, got {line}"
        );

        // None (no ledger coverage) selects no flavor at all -> mood line.
        let no_coverage = dawn_day(None, true);
        let line =
            current_pet_speech_for_scene(Mood::Content, &profile, &no_coverage, visible).unwrap();
        assert!(
            content.contains(&line.as_str()),
            "None must fall through to the mood line, got {line}"
        );

        // Maturity gate: baseline-ratio channels stay silent while immature.
        let immature = dawn_day(
            Some(DaySummary {
                ratio: 0.0,
                dominant_shape: None,
            }),
            false,
        );
        let line =
            current_pet_speech_for_scene(Mood::Content, &profile, &immature, visible).unwrap();
        assert!(
            content.contains(&line.as_str()),
            "immature must fall through to the mood line, got {line}"
        );
    }

    #[test]
    fn dream_windows_are_deterministic_and_need_yesterdays_shape_detail() {
        use crate::tui::day::{DayContext, DaySummary};
        use crate::tui::life::WorkWeather;
        // Base: an hour boundary on a visible slot of an eligible 3rd cycle
        // (unix 1778500800 is divisible by 90). 40 probes at 90s steps cover
        // one hour, all on eligible visible slots, all in local hour 12.
        let base = datetime!(2026-05-11 12:00 UTC);
        let sparks_day = DayContext {
            asleep: true,
            date_seed: 7,
            yesterday: Some(DaySummary {
                ratio: 1.2,
                dominant_shape: Some(WorkWeather::OutputSparks),
            }),
            ..Default::default()
        };
        let profile = crate::tui::life::PetLifeProfile::default();
        let scan = |day: &DayContext| -> Vec<bool> {
            (0..40_i64)
                .map(|k| {
                    let at = base + time::Duration::seconds(k * 90);
                    match current_pet_speech_for_scene(Mood::Content, &profile, day, at) {
                        Some(line) => {
                            assert!(
                                SLEEP_SPEECH_PHRASES.contains(&line.as_str())
                                    || DREAM_SPARKS_PHRASES.contains(&line.as_str()),
                                "asleep lines are zzz or this family's dreams only, got {line}"
                            );
                            DREAM_SPARKS_PHRASES.contains(&line.as_str())
                        }
                        None => panic!("every probe sits on an eligible visible slot"),
                    }
                })
                .collect()
        };
        let pass1 = scan(&sparks_day);
        let pass2 = scan(&sparks_day);
        assert_eq!(pass1, pass2, "dream windows must be restart-deterministic");
        let dream_probes = pass1.iter().filter(|&&d| d).count();
        assert!(
            (5..=8).contains(&dream_probes),
            "one ~10-minute window sampled every 90s, got {dream_probes}"
        );
        // One contiguous window: zzz on either side, dreams in the middle.
        let first = pass1.iter().position(|&d| d).unwrap();
        let last = pass1.iter().rposition(|&d| d).unwrap();
        assert!(
            pass1[first..=last].iter().all(|&d| d),
            "dream probes must form one contiguous window"
        );

        // No signal -> no dreams, zzz only (honesty rule): uncovered,
        // shape-less, and Clear (no dominant character) yesterdays.
        for yesterday in [
            None,
            Some(DaySummary {
                ratio: 0.7,
                dominant_shape: None,
            }),
            Some(DaySummary {
                ratio: 0.7,
                dominant_shape: Some(WorkWeather::Clear),
            }),
        ] {
            let day = DayContext {
                yesterday,
                ..sparks_day
            };
            let any_dream = scan(&day).into_iter().any(|d| d);
            assert!(
                !any_dream,
                "no signal must mean zzz only, got a dream for {yesterday:?}"
            );
        }
    }
```

- [ ] **Step 4: Run, verify compile-red**

Run: `cargo test --lib pet::speech 2>&1 | head -30`
Expected: `E0308: mismatched types` / `E0061` — `current_pet_speech_for_scene` still takes `asleep: bool` where the tests pass `&DayContext`; `MORNING_RESTED_PHRASES` / `DREAM_SPARKS_PHRASES` not found.

- [ ] **Step 5: Implement the stack and update BOTH callers**

In `src/pet/speech.rs`, delete `current_pet_speech_for_profile` (speech.rs:47-62 — its body is absorbed below) and replace everything from the `SLEEP_SPEECH_CYCLE_N` block through the end of `current_pet_speech_for_scene` (speech.rs:64-90) with:

```rust
use crate::tui::life::WorkWeather;

/// Show the sleep bubble only on every Nth 30s speech cycle — night is calm.
const SLEEP_SPEECH_CYCLE_N: i64 = 3;
const SLEEP_SPEECH_PHRASES: &[&str] = &["zzz...", "...zzz", "z z z"];

/// Length of each deterministic per-hour dream window. The clock only picks
/// the moment (locked rule: wall clock varies texture, never content); the
/// dream family comes from yesterday's real shape signal.
const DREAM_WINDOW_MINUTES: i64 = 10;

/// Dream pools per yesterday's dominant shape family (misty / sparking /
/// pulsing). Authoring guardrail: the pet's own dream imagery only.
const DREAM_MIST_PHRASES: &[&str] = &["*dreams of drifting mist*", "*soft fog rolls past*"];
const DREAM_SPARKS_PHRASES: &[&str] = &["*dreams of tiny sparks*", "*sparks flicker by*"];
const DREAM_PULSE_PHRASES: &[&str] = &["*dreams in slow pulses*", "*a gentle pulse hums*"];
const DREAM_MIXED_PHRASES: &[&str] = &["*dreams of swirling colors*", "*a busy little dream*"];

/// Morning-after flavor thresholds on yesterday's baseline ratio: a feast
/// day reads mellow, an observed idle day reads rested, in between reads
/// fresh. The feast threshold is the SHARED `crate::tui::day::FEAST_DAY_RATIO`
/// (Task 10's prop resonance uses the same notion of "yesterday was a
/// feast") — as part of this step, add it to the day.rs constants block
/// (after `PHASE_BLEND_MINUTES`):
///
///   /// A finished day reads as a feast when its ratio clears this multiple
///   /// of the calibration baseline.
///   pub const FEAST_DAY_RATIO: f32 = 1.5;
const MORNING_IDLE_RATIO: f32 = 0.1;
/// Authoring guardrail (binding): every morning line expresses the pet's OWN
/// state — rested, mellow, content — never the user's absence, yesterday's
/// lowness, or owed make-up work.
const MORNING_MELLOW_PHRASES: &[&str] = &[
    "*stretches* still full...",
    "what a feast that was",
    "slow and cozy this morning",
];
const MORNING_RESTED_PHRASES: &[&str] = &[
    "*stretches* feeling rested!",
    "bright-eyed this morning",
    "good morning!",
];
const MORNING_FRESH_PHRASES: &[&str] = &["morning!", "*happy wiggle* a new day", "fresh and ready"];

/// Scene-aware speech selector: the binding precedence stack (top wins; the
/// petting override sits above this at the app layer):
///   1. asleep — dream line during a dream window when yesterday carries
///      shape detail, else the sparse zzz cadence
///   2. live-burst munch
///   3. needy mood (Hungry/Sad/Wilted — the sanctioned vitals signal always
///      outranks flavor)
///   4. morning-after greeting flavor (maturity-gated; a `None` yesterday —
///      no ledger coverage — selects no flavor at all)
///   5. default mood line
pub fn current_pet_speech_for_scene(
    mood: Mood,
    profile: &crate::tui::life::PetLifeProfile,
    day: &crate::tui::day::DayContext,
    now: OffsetDateTime,
) -> Option<String> {
    if day.asleep {
        let cycle_pos = now.unix_timestamp().rem_euclid(SPEECH_CYCLE_SECS);
        let cycle_index = now.unix_timestamp().div_euclid(SPEECH_CYCLE_SECS);
        if cycle_pos >= SPEECH_VISIBLE_SECS || cycle_index.rem_euclid(SLEEP_SPEECH_CYCLE_N) != 0 {
            return None;
        }
        // Dreams only when yesterday carries real shape detail (honesty rule:
        // no signal -> no dreams, zzz only).
        if in_dream_window(day.date_seed, now) {
            if let Some(line) = day
                .yesterday
                .and_then(|y| y.dominant_shape)
                .and_then(|shape| dream_phrase(shape, now))
            {
                return Some(line);
            }
        }
        let idx = cycle_index
            .div_euclid(SLEEP_SPEECH_CYCLE_N)
            .rem_euclid(SLEEP_SPEECH_PHRASES.len() as i64) as usize;
        return Some(SLEEP_SPEECH_PHRASES[idx].to_string());
    }

    let cycle_pos = now.unix_timestamp().rem_euclid(SPEECH_CYCLE_SECS);
    if cycle_pos >= SPEECH_VISIBLE_SECS {
        return None;
    }
    if profile.burst_level >= 0.35 || profile.activity_level >= 1.25 {
        return Some(pick_munch_phrase(now));
    }
    // The sanctioned vitals signal always outranks flavor: two channels both
    // keyed to the user working less must never stack into nagging.
    if matches!(mood, Mood::Hungry | Mood::Sad | Mood::Wilted) {
        return Some(mood_phrase(mood, now));
    }
    // Morning-after is a baseline-ratio channel: maturity-gated, and a None
    // yesterday (no ledger coverage) selects no flavor at all.
    if day.mature && crate::tui::day::in_morning_after_window(day, now) {
        if let Some(yesterday) = day.yesterday {
            return Some(morning_after_phrase(yesterday, now));
        }
    }
    Some(mood_phrase(mood, now))
}

/// Deterministic dream window: each hour holds one DREAM_WINDOW_MINUTES
/// window at a minute offset hashed from (date_seed, hour).
fn in_dream_window(date_seed: u64, now: OffsetDateTime) -> bool {
    let hour = u64::from(now.hour());
    let mixed = (date_seed ^ hour.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_mul(0x0000_0100_0000_01B3);
    let offset = (mixed % (60 - DREAM_WINDOW_MINUTES) as u64) as i64;
    let minute = i64::from(now.minute());
    minute >= offset && minute < offset + DREAM_WINDOW_MINUTES
}

/// Dream line for yesterday's dominant shape family. `Clear` carries no
/// dominant character, so it dreams nothing (the zzz cadence covers it).
fn dream_phrase(shape: WorkWeather, now: OffsetDateTime) -> Option<String> {
    let phrases: &[&str] = match shape {
        WorkWeather::CacheMist => DREAM_MIST_PHRASES,
        WorkWeather::OutputSparks => DREAM_SPARKS_PHRASES,
        WorkWeather::ReasoningPulse => DREAM_PULSE_PHRASES,
        WorkWeather::Mixed => DREAM_MIXED_PHRASES,
        WorkWeather::Clear => return None,
    };
    let idx = (now.unix_timestamp() / SPEECH_CYCLE_SECS).rem_euclid(phrases.len() as i64) as usize;
    Some(phrases[idx].to_string())
}

/// Morning-after greeting flavored by yesterday's real ratio.
fn morning_after_phrase(yesterday: crate::tui::day::DaySummary, now: OffsetDateTime) -> String {
    let phrases: &[&str] = if yesterday.ratio >= crate::tui::day::FEAST_DAY_RATIO {
        MORNING_MELLOW_PHRASES
    } else if yesterday.ratio <= MORNING_IDLE_RATIO {
        MORNING_RESTED_PHRASES
    } else {
        MORNING_FRESH_PHRASES
    };
    let idx = (now.unix_timestamp() / SPEECH_CYCLE_SECS).rem_euclid(phrases.len() as i64) as usize;
    phrases[idx].to_string()
}
```

(Place the `use crate::tui::life::WorkWeather;` line with the existing imports at the top of the file, next to `use crate::game::metabolism::Mood;`.)

**Caller 1 — `src/tui/app.rs:569-574`** (Read install_poll_result at 551-578 first). Replace:

```rust
        result.vm.current_speech = crate::pet::speech::current_pet_speech_for_scene(
            result.vm.pet_render.mood,
            &result.vm.life_profile,
            result.vm.day_context.asleep,
            now,
        );
```

with:

```rust
        result.vm.current_speech = crate::pet::speech::current_pet_speech_for_scene(
            result.vm.pet_render.mood,
            &result.vm.life_profile,
            &result.vm.day_context,
            now,
        );
```

**Caller 2 — `src/commands/watch.rs:175-188`** (Read first). Replace the whole `current_speech: if day_context.asleep { ... } else { ... },` field with:

```rust
        current_speech: crate::pet::speech::current_pet_speech_for_scene(
            mood,
            &crate::tui::life::PetLifeProfile::default(),
            &day_context,
            now,
        ),
```

(`day_context` is `Copy`; the local stays usable after the `day_context,` field init at watch.rs:150.)

Then remove the now-dead raw-token path from `src/commands/watch.rs`: the `RECENT_ACTIVITY_WINDOW` const + doc comment and `fn recent_activity_tokens` (watch.rs:272-290), and the test `recent_activity_tokens_uses_bucket_at_not_observed_at` (watch.rs:~934-960, including its test-local `event` closure). Without this, `cargo clippy -D warnings` fails on dead code.

- [ ] **Step 6: Run tests, verify green**

Run: `cargo test --lib pet::speech -- --nocapture`
Expected: 0 failed; the 3 rewritten + 3 new speech tests pass.

Run: `cargo test --lib vm_build_speech_uses_the_scene_precedence_stack`
Expected: 1 passed.

Run: `cargo test 2>&1 | tail -5`
Expected: green. **If `tests/dev_preview.rs` insta snapshots fail:** the vm-build speech line in the pinned frames legitimately changed (legacy raw-token munch -> precedence stack with the default profile). Inspect the snapshot diff — the ONLY change must be the speech-bubble text — then accept with `cargo insta accept` (or `cargo insta review`) and include `tests/snapshots/*.snap` in the commit. Any other frame change is a defect: stop and debug.

- [ ] **Step 7: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add src/pet/speech.rs src/tui/app.rs src/commands/watch.rs
# plus tests/snapshots/*.snap if step 6 accepted a speech-only frame change
git commit -m "feat(pet): route speech through the full scene precedence stack

Dreams (yesterday's shape family, date_seed-timed windows), morning-after
flavor (yesterday.ratio, maturity-gated, pet's-own-state lines only), needy
vitals outrank all flavor. Supersedes the vm-build raw-token munch path:
removes recent_activity_tokens, RECENT_ACTIVITY_WINDOW, and their test."
```

---

### Task 6: tiredness motion — blink slowdown + tired breath rhythm

**Spec sections:** Branch T2 — "Evening tiredness (its own vocabulary, not droop)": timing-level cues only (blink cadence slows, breath period lengthens), the energy vital and droop shader untouched, timing cues survive Flat. Interface sheet: `AnimationFrame.blink_slowdown` (Copy+Eq, u8), `BreathRhythm::Tired { eighths }`, `TIRED_BLINK_MAX_SLOWDOWN=24` (render.rs), `TIRED_BREATH_MAX_SCALE=1.5` (animator.rs), wiring rules "Breath call sites pick: asleep -> Asleep{onset}; else tiredness > 0.05 -> Tired{eighths}; else Awake" and "blink_slowdown is computed by callers from vm.day_context.tiredness ... at the SAME call sites T1 touched (watch build, app frame tick, menubar animate)".

**Call-site map (how "the same three call sites" resolve):** the watch vm build constructs `AnimationFrame` directly (watch.rs:77-86); the app frame tick (app.rs:266) and menubar animate (menubar/app.rs:434) both flow through `rerender_pet_for_view_model` (watch.rs:436-456), which constructs the frame from `&mut WatchViewModel` — so computing `blink_slowdown` from `vm.day_context.tiredness` inside it covers both, and its signature gains nothing (interface sheet rule). Breath has two call sites: the vm build (watch.rs:190-196) and the app frame tick (app.rs:273-284); the menubar popover has no breath surface (T1 scope note). Both breath sites share one mapping helper so they cannot drift.

**Files:**
- Modify: `src/pet/render.rs` (`AnimationFrame` at 6-13; `should_blink` cadence at 254-257; new const + mapping helper; tests at 614-653)
- Modify: `src/pet/animator.rs` (`BreathRhythm` at 284-292; `compute_breath_offset_with_rhythm` match at 300-307; new const + `breath_rhythm_for_day`; tests)
- Modify: `src/commands/watch.rs` (frame at 77-86; breath at 190-196; rerender frame at 443-452)
- Modify: `src/tui/app.rs` (rhythm match at 273-279)
- Modify (field-add only, `blink_slowdown: 0`): `src/tui/layout.rs:346`, `src/tui/panels/pet.rs:1297`, `src/dev_preview/pets.rs:69`
- Test: in-module tests in `render.rs`, `animator.rs`, plus two wiring tests in `watch.rs`

**Anchor warnings:** Read `src/pet/render.rs:6-13` + `:242-258`, `src/pet/animator.rs:284-315`, `src/commands/watch.rs:77-86` + `:190-196` + `:436-456`, `src/tui/app.rs:258-286`, and each field-add site (`src/tui/layout.rs:342-351`, `src/tui/panels/pet.rs:1293-1302`, `src/dev_preview/pets.rs:62-74`) before editing. Do NOT touch `AnimationProfile.breath_*` (render.rs:56-62) — vestigial for breathing per the spec; only `blink_average`/`blink_jitter` participate in the blink seam. Depends on Task 4 (`day_context.tiredness`).

- [ ] **Step 1: Write the behaviorally-failing rerender wiring test (compiles against old code, fails red)**

Append to `mod tests` in `src/commands/watch.rs`:

```rust
    #[test]
    fn rerender_threads_day_context_tiredness_into_blink_cadence() {
        let mut rested = WatchViewModel::fixture();
        rested.pet_render.mood = Mood::Content;
        rested.pet_render.generated_species = Species::Blob;
        let mut tired = rested.clone();
        tired.day_context.tiredness = 1.0;

        let closed = crate::pet::render::closed_blink_eyes(Species::Blob);
        let mut rested_blinks = 0;
        let mut tired_blinks = 0;
        for tick in 0..600_u64 {
            rerender_pet_for_view_model(&mut rested, tick, false).unwrap();
            if rested.pet_art.join("\n").contains(closed) {
                rested_blinks += 1;
            }
            rerender_pet_for_view_model(&mut tired, tick, false).unwrap();
            if tired.pet_art.join("\n").contains(closed) {
                tired_blinks += 1;
            }
        }
        assert!(tired_blinks > 0, "a tired pet still blinks, just less often");
        assert!(
            rested_blinks > tired_blinks,
            "tiredness must slow blinking through the rerender path \
             (app frame tick + menubar animate): {rested_blinks} vs {tired_blinks}"
        );
    }
```

- [ ] **Step 2: Run it, verify it fails for the right reason**

Run: `cargo test --lib rerender_threads_day_context_tiredness -- --nocapture`
Expected: FAILED with equal counts (`rested_blinks > tired_blinks: N vs N`) — the rerender path ignores tiredness today.

- [ ] **Step 3: Write the failing unit + vm tests (compile-red: new field/variant)**

Append to `mod tests` in `src/pet/render.rs`:

```rust
    #[test]
    fn blink_slowdown_maps_tiredness_zero_to_zero_and_full_to_max() {
        assert_eq!(blink_slowdown_for_tiredness(0.0), 0);
        assert_eq!(blink_slowdown_for_tiredness(1.0), TIRED_BLINK_MAX_SLOWDOWN);
        assert_eq!(
            blink_slowdown_for_tiredness(0.5),
            TIRED_BLINK_MAX_SLOWDOWN / 2
        );
        // Out-of-range inputs clamp instead of wrapping.
        assert_eq!(blink_slowdown_for_tiredness(7.0), TIRED_BLINK_MAX_SLOWDOWN);
        assert_eq!(blink_slowdown_for_tiredness(-1.0), 0);
    }

    #[test]
    fn blink_cadence_slows_monotonically_with_blink_slowdown() {
        use crate::pet::generation::Species;
        // Non-glitch species: corruption must not perturb eye-glyph detection.
        let pet = generate_pet("hold-eyes-seed").with_species(Species::Blob);
        let blink_count = |slowdown: u8| {
            (0..1500_u64)
                .filter(|&tick| {
                    let rendered = render_pet(
                        &pet,
                        Stage::S3,
                        Mood::Content,
                        AnimationFrame {
                            tick,
                            blink_suppression_ticks: 0,
                            hold_eyes_closed: false,
                            blink_slowdown: slowdown,
                        },
                    );
                    rendered
                        .lines
                        .join("\n")
                        .contains(closed_blink_eyes(pet.species))
                })
                .count()
        };
        let rested = blink_count(0);
        let halfway = blink_count(TIRED_BLINK_MAX_SLOWDOWN / 2);
        let exhausted = blink_count(TIRED_BLINK_MAX_SLOWDOWN);
        assert!(rested > 0, "a rested pet blinks");
        assert!(exhausted > 0, "a tired pet still blinks, just slower");
        assert!(
            rested > halfway && halfway > exhausted,
            "cadence must slow monotonically: {rested} > {halfway} > {exhausted}"
        );
    }
```

Update the two existing `AnimationFrame` literals in render.rs tests (`hold_eyes_closed_renders_closed_blink_eyes_without_touching_mood` at :622-626 and `hold_eyes_closed_false_keeps_existing_blink_behavior` at :642-646): add `blink_slowdown: 0,` after `hold_eyes_closed`.

Append to `mod tests` in `src/pet/animator.rs`:

```rust
    #[test]
    fn tired_breath_period_scale_at_full_eighths_equals_tired_breath_max_scale() {
        // Crystal awake period: 6.0s (60 decis). Count inhale onsets (0->1
        // edges) over 180s at 0.1s resolution: 30 awake cycles vs 20 tired
        // cycles is exactly the TIRED_BREATH_MAX_SCALE = 1.5 period stretch.
        let species = Some(Species::Crystal);
        let rising_edges = |rhythm: BreathRhythm| {
            let base = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
            let mut prev = 0;
            let mut edges = 0;
            for ds in 0..1800_i64 {
                let now = base + time::Duration::milliseconds(ds * 100);
                let cur = compute_breath_offset_with_rhythm(species, now, rhythm);
                if prev == 0 && cur == 1 {
                    edges += 1;
                }
                prev = cur;
            }
            edges
        };
        let awake = rising_edges(BreathRhythm::Awake);
        let tired = rising_edges(BreathRhythm::Tired { eighths: 8 });
        assert_eq!(awake, 30);
        assert_eq!(tired, 20);
        assert!(
            (f64::from(awake) / f64::from(tired) - TIRED_BREATH_MAX_SCALE).abs() < f64::EPSILON,
            "full-eighths period stretch must equal TIRED_BREATH_MAX_SCALE"
        );
    }

    #[test]
    fn breath_rhythm_for_day_picks_asleep_over_tired_over_awake() {
        let onset = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        // Asleep outranks tired: a fully tired pet still breathes sleep.
        let asleep = crate::tui::day::DayContext {
            asleep: true,
            sleep_onset_utc: Some(onset),
            tiredness: 1.0,
            ..Default::default()
        };
        assert_eq!(
            breath_rhythm_for_day(&asleep),
            BreathRhythm::Asleep { onset }
        );
        // Tired only above the 0.05 activation floor; eighths = round(t * 8).
        let tired = crate::tui::day::DayContext {
            tiredness: 0.5,
            ..Default::default()
        };
        assert_eq!(
            breath_rhythm_for_day(&tired),
            BreathRhythm::Tired { eighths: 4 }
        );
        let barely = crate::tui::day::DayContext {
            tiredness: 0.05,
            ..Default::default()
        };
        assert_eq!(breath_rhythm_for_day(&barely), BreathRhythm::Awake);
        assert_eq!(
            breath_rhythm_for_day(&crate::tui::day::DayContext::default()),
            BreathRhythm::Awake
        );
    }
```

Append to `mod tests` in `src/commands/watch.rs` (the asleep-outranks-tired wiring test, driven through the real `build_watch_view_model_at` path — ledger-derived context, no hand-set internals):

```rust
    #[test]
    fn vm_breath_rhythm_lets_asleep_outrank_tiredness() {
        use crate::pet::animator::{compute_breath_offset_with_rhythm, BreathRhythm};
        use time::macros::datetime;
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        let mut store = UsageStore::open(&db_path).unwrap();
        // Mature ledger: 5 prior days at hours 09/13/17 (June 4-8), plus a
        // heavy evening June 9 18:00-21:50 (24 buckets, 480k tokens). The
        // derived night window is 0..7 local, so at 01:30 June 10 the pet is
        // asleep (220 min ledger-quiet) AND tired (evening inside the 16h
        // fatigue window => tiredness 0.25 > 0.05).
        for back in 2..=6_i64 {
            for hour in [9_i64, 13, 17] {
                let at = datetime!(2026-06-10 00:00 UTC) - Duration::days(back)
                    + Duration::hours(hour);
                store
                    .insert_event(&sample_event_at_for_test(at, 10_000.0))
                    .unwrap();
            }
        }
        for i in 0..24_i64 {
            store
                .insert_event(&sample_event_at_for_test(
                    datetime!(2026-06-09 18:00 UTC) + Duration::minutes(i * 10),
                    20_000.0,
                ))
                .unwrap();
        }
        drop(store);
        let mut state = PetState::new_for_test("test", "buddy");
        state.created_at = datetime!(2026-06-01 00:00 UTC);
        state.pet.generated_species = Species::Crystal;

        // Sleep onset: max(last bucket 21:50 + 20min idle, night start 00:00)
        // = June 10 00:00. Probe an instant where the asleep and tired
        // rhythms disagree, so the assertion can't pass by coincidence.
        let now = datetime!(2026-06-10 01:30 UTC);
        let onset = datetime!(2026-06-10 00:00 UTC);
        let asleep_rhythm = BreathRhythm::Asleep { onset };
        let tired_rhythm = BreathRhythm::Tired { eighths: 2 };
        let probe = (0..180_i64)
            .map(|s| now + Duration::seconds(s))
            .find(|&t| {
                compute_breath_offset_with_rhythm(Some(Species::Crystal), t, asleep_rhythm)
                    != compute_breath_offset_with_rhythm(Some(Species::Crystal), t, tired_rhythm)
            })
            .expect("sleep (18s period) and tired (6.7s) rhythms diverge fast");

        let vm = build_watch_view_model_at(
            &state,
            &db_path,
            probe,
            LocalDayMapper::Fixed(time::UtcOffset::UTC),
        )
        .unwrap();
        assert!(vm.day_context.asleep, "fixture must derive an asleep scene");
        assert!(
            vm.day_context.tiredness > 0.05,
            "fixture must also be tired, got {}",
            vm.day_context.tiredness
        );
        assert_eq!(
            vm.breath_offset_y,
            compute_breath_offset_with_rhythm(Some(Species::Crystal), probe, asleep_rhythm),
            "asleep must outrank tired at the vm breath call site"
        );
    }
```

- [ ] **Step 4: Run, verify compile-red**

Run: `cargo test --lib pet:: 2>&1 | head -30`
Expected: `E0560: struct AnimationFrame has no field named blink_slowdown`, `E0599: no variant named Tired found for enum BreathRhythm`, `E0425: cannot find function blink_slowdown_for_tiredness / breath_rhythm_for_day`.

- [ ] **Step 5: Implement render.rs + animator.rs + every construction site**

`src/pet/render.rs` — add the field to `AnimationFrame` (render.rs:6-13), doc comment verbatim from the sheet (struct stays `Copy + Eq`, integer field):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationFrame {
    pub tick: u64,
    pub blink_suppression_ticks: u8,
    /// Sleep presentation: force the species closed-blink eyes. Must never be
    /// implemented by substituting Mood::Sleepy — mood is the vitals contract.
    pub hold_eyes_closed: bool,
    /// Ticks added to the species blink cadence (tiredness slows blinking).
    /// 0 = normal. Producers map tiredness 0..1 -> 0..TIRED_BLINK_MAX_SLOWDOWN.
    pub blink_slowdown: u8,
}
```

Add the constant + producer mapping (next to `closed_blink_eyes`, render.rs:168):

```rust
/// Ticks added to the species blink cadence at tiredness 1.0 — a tired pet
/// blinks slower, never faster. Timing cues survive Flat (spec: tiredness).
pub const TIRED_BLINK_MAX_SLOWDOWN: u8 = 24;

/// Producer mapping: tiredness 0..=1 -> 0..=TIRED_BLINK_MAX_SLOWDOWN ticks,
/// clamped. Shared by every AnimationFrame producer so call sites can't drift.
pub fn blink_slowdown_for_tiredness(tiredness: f32) -> u8 {
    (tiredness.clamp(0.0, 1.0) * f32::from(TIRED_BLINK_MAX_SLOWDOWN)).round() as u8
}
```

In `should_blink` (render.rs:254-257 — Read first), extend the cadence sum:

```rust
    let jitter = u64::from(profile.blink_jitter.max(1));
    let cadence = u64::from(profile.blink_average)
        + (u64::from(pet.animation_phase.blink) % jitter)
        + u64::from(frame.blink_slowdown);
    (frame.tick + u64::from(pet.animation_phase.blink)).is_multiple_of(cadence)
```

(Note: `Sad | Sleepy | Wilted` moods already return early above — tiredness never collides with mood-suppressed blinking, and `hold_eyes_closed` short-circuits `should_blink` entirely, so asleep frames are unaffected.)

`src/pet/animator.rs` — constants next to the sleep scales (animator.rs:53-55):

```rust
/// Tired breath: period multiplier at tiredness 1.0 (eighths = 8). The rhythm
/// math stretches in integer sixteenths — (16 + eighths) / 16 — so full
/// eighths is exactly this scale.
pub const TIRED_BREATH_MAX_SCALE: f64 = 1.5;
/// Tiredness must exceed this floor before the breath period lengthens
/// (sheet wiring rule: "else tiredness > 0.05 -> Tired{eighths}").
const TIRED_BREATH_MIN_TIREDNESS: f32 = 0.05;
```

New variant (animator.rs:284-292), doc comment verbatim from the sheet:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreathRhythm {
    Awake,
    /// Slowed cycle whose phase is anchored at the sleep-onset instant so the
    /// period change is continuous, not a pop.
    Asleep {
        onset: time::OffsetDateTime,
    },
    /// Lengthened period for a tired-but-awake pet. eighths in 0..=8 maps to
    /// period scale 1.0..=TIRED_BREATH_MAX_SCALE (integer to keep Copy+Eq).
    Tired { eighths: u8 },
}
```

New match arm in `compute_breath_offset_with_rhythm` (animator.rs:300-307 — Read first), after the `Asleep` arm:

```rust
        BreathRhythm::Tired { eighths } => {
            // Period stretch in integer sixteenths: 16/16 at eighths=0 up to
            // 24/16 = TIRED_BREATH_MAX_SCALE at eighths=8. Inhale window and
            // anchor stay awake-shaped — tired is slower, not deeper.
            let stretch = 16 + i64::from(eighths.min(8));
            (period_ds * stretch / 16, inhale_ds, 0)
        }
```

Caller mapping helper, after `compute_breath_offset_with_rhythm`:

```rust
/// Scene mapping rule (binding): asleep -> Asleep{onset}; else tiredness
/// above the floor -> Tired{eighths}; else Awake. Shared by the watch vm
/// build and the app frame tick so the two breath call sites can't drift.
pub fn breath_rhythm_for_day(day: &crate::tui::day::DayContext) -> BreathRhythm {
    if let (true, Some(onset)) = (day.asleep, day.sleep_onset_utc) {
        return BreathRhythm::Asleep { onset };
    }
    if day.tiredness > TIRED_BREATH_MIN_TIREDNESS {
        return BreathRhythm::Tired {
            eighths: (day.tiredness.clamp(0.0, 1.0) * 8.0).round() as u8,
        };
    }
    BreathRhythm::Awake
}
```

**Construction/call sites** (Read each region before editing):

1. `src/commands/watch.rs:77-86` (vm build frame) — add after `hold_eyes_closed: day_context.asleep,`:

```rust
            blink_slowdown: crate::pet::render::blink_slowdown_for_tiredness(
                day_context.tiredness,
            ),
```

2. `src/commands/watch.rs:190-196` (vm build breath) — replace the `breath_offset_y: { ... }` block with:

```rust
        breath_offset_y: crate::pet::animator::compute_breath_offset_with_rhythm(
            Some(species),
            now,
            crate::pet::animator::breath_rhythm_for_day(&day_context),
        ),
```

3. `src/commands/watch.rs:443-452` (`rerender_pet_for_view_model` frame — covers the app frame tick AND menubar animate; its signature gains nothing, per the sheet) — add after `hold_eyes_closed,`:

```rust
            blink_slowdown: crate::pet::render::blink_slowdown_for_tiredness(
                vm.day_context.tiredness,
            ),
```

4. `src/tui/app.rs:271-279` (`advance_animation_frame` breath) — replace:

```rust
        let rhythm = match (
            self.vm.day_context.asleep,
            self.vm.day_context.sleep_onset_utc,
        ) {
            (true, Some(onset)) => crate::pet::animator::BreathRhythm::Asleep { onset },
            _ => crate::pet::animator::BreathRhythm::Awake,
        };
```

with:

```rust
        let rhythm = crate::pet::animator::breath_rhythm_for_day(&self.vm.day_context);
```

5. Field-add `blink_slowdown: 0,` to the remaining `AnimationFrame` literals (test/preview producers with no tiredness source): `src/tui/layout.rs:346-350`, `src/tui/panels/pet.rs:1297-1301`, `src/dev_preview/pets.rs:69-73`.

No menubar diff: `src/menubar/app.rs:430-436` already routes through `rerender_pet_for_view_model`, which now reads `vm.day_context.tiredness` itself.

- [ ] **Step 6: Run tests, verify green**

Run: `cargo test --lib pet:: -- --nocapture`
Expected: 0 failed; `blink_slowdown_maps...`, `blink_cadence_slows_monotonically...`, `tired_breath_period_scale...`, `breath_rhythm_for_day_picks...` all pass.

Run: `cargo test --lib rerender_threads_day_context_tiredness && cargo test --lib vm_breath_rhythm_lets_asleep_outrank_tiredness`
Expected: both pass.

Run: `cargo test 2>&1 | tail -5`
Expected: full suite green. Preview frames must NOT change in this task (fixture day contexts default to `tiredness: 0.0` and the scratch ledgers are immature, so `blink_slowdown` is 0 everywhere in Preview Lab); if a dev_preview snapshot fails here, that is a defect — stop and debug, do not accept.

- [ ] **Step 7: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add src/pet/render.rs src/pet/animator.rs src/commands/watch.rs src/tui/app.rs \
        src/tui/layout.rs src/tui/panels/pet.rs src/dev_preview/pets.rs
git commit -m "feat(pet): slow blink and breath with accumulated tiredness

AnimationFrame.blink_slowdown (0..TIRED_BLINK_MAX_SLOWDOWN ticks) and
BreathRhythm::Tired{eighths} (period x1.0..x1.5 in integer sixteenths),
mapped from day_context.tiredness at the T1 call sites; asleep outranks
tired outranks awake. Timing cues only — droop and the energy vital are
untouched and the cues survive Flat."
```
<!-- Part C: Tasks 7-9 (the scene) — day-accumulation motes, today's sky character + climate + seasons, weekend texture. Stitch after the pet-layer tasks (Tasks 4-6). -->

### Task 7: Day-accumulation floor motes (soft saturation, budget cap, tidy fade)

**Spec sections:** "Branch T2 — Day accumulation" (soft saturation, sub-countable, `date_seed` jitter, `MOTE_BUDGET_SHARE` cap, maturity gate, no numbers / no fill direction / no completion framing) and "Boundary behavior" (motes fade out over `MOTE_TIDY_FADE_MINUTES` after local-day rollover).

**Interface sheet bindings:** `mote_glyphs_for(day, habitat, exclusions, now, color_capability) -> Vec<AmbientGlyph>` in `src/tui/panels/pet.rs`, rendered in its own pass after ambient / before activity glyphs with the same exclusions; `MOTE_BUDGET_SHARE = 0.5` (pet.rs); `MOTE_TIDY_FADE_MINUTES = 30` (day.rs); `DayContext.local_day_started_utc` (day.rs) is the tidy-fade anchor; Flat → zero motes; gated on `day.mature`.

**Dependency note:** the interface sheet groups `local_day_started_utc` with `tiredness` under "DayContext additions". If the tiredness task earlier in this plan already added the field, **skip the three `local_day_started_utc` edits in Step 3** — verify first with `grep -n local_day_started_utc src/tui/day.rs`. The stamping test in Step 1 stays either way (it pins behavior, not authorship).

**Files:**
- Modify: `src/tui/day.rs` (`MOTE_TIDY_FADE_MINUTES` constant near `PHASE_BLEND_MINUTES` at day.rs:35; `DayContext` field at day.rs:179-200; `Default` at day.rs:202-224; `build_day_context` literal at day.rs:353-373)
- Modify: `src/tui/panels/pet.rs` (constants below `PET_H` at pet.rs:32; `mote_glyphs_for` + `mote_density` after `ambient_glyphs_for_phase` ends at pet.rs:462; render pass between the ambient paint loop ending at pet.rs:659 and `let compact = ...` at pet.rs:660)
- Test: in-module `#[cfg(test)]` tests at the bottom of both files (day.rs tests mod starts at day.rs:535; pet.rs tests mod starts at pet.rs:1271, existing sky tests end at pet.rs:2141)

- [ ] **Step 1: Write the failing tests**

In `src/tui/day.rs` tests mod (append after the existing `day_shape_classification_...` test; helpers `utc_mapper` / `store_with_applied` already exist at day.rs:542-558):

```rust
    #[test]
    fn day_context_carries_the_local_day_started_instant() {
        let now = datetime!(2026-06-09 15:00 UTC);
        let store = store_with_applied(&[(now - time::Duration::hours(1), 5_000.0)]);
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(2);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert_eq!(ctx.local_day_started_utc, datetime!(2026-06-09 00:00 UTC));
        assert_eq!(ctx.local_day_rollover_utc, datetime!(2026-06-10 00:00 UTC));
        assert_eq!(
            DayContext::default().local_day_started_utc,
            time::OffsetDateTime::UNIX_EPOCH
        );
    }
```

In `src/tui/panels/pet.rs` tests mod (append before the closing brace at pet.rs:2142):

```rust
    #[test]
    fn mote_density_soft_saturates_with_no_learnable_full_state() {
        // Sub-countable, asymptotic: the 2.0 -> 4.0 step must be smaller than
        // the 0.0 -> 1.0 step (no visually distinct "full room" to learn) but
        // still rising (no hard cap to learn either), and it never reaches 1.
        let step01 = mote_density(1.0) - mote_density(0.0);
        let step24 = mote_density(4.0) - mote_density(2.0);
        assert!(
            step24 < step01,
            "saturating: step24 {step24} must be < step01 {step01}"
        );
        assert!(mote_density(4.0) > mote_density(2.0), "still rising");
        assert!(mote_density(100.0) < 1.0, "asymptotic, never full");
        assert_eq!(mote_density(0.0), 0.0, "no work, no motes");
    }

    #[test]
    fn motes_cap_at_the_budget_share_of_the_ambient_allocation() {
        // 40x12 habitat: cells = 480, area_term = (480-200)/60 = 4, Day phase
        // scale 1.0, stage-floor allocation = 4 + 4 = 8, budget = floor(0.5*8)
        // = 4. Even at an absurd ratio the count stays at or under the cap.
        let habitat = Rect::new(0, 0, 40, 12);
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let day = crate::tui::day::DayContext {
            mature: true,
            today_ratio: 100.0,
            ..crate::tui::day::DayContext::default()
        };
        let motes = mote_glyphs_for(&day, habitat, &[], now, ColorCapability::Truecolor);
        assert!(!motes.is_empty(), "a heavy day shows motes");
        assert!(motes.len() <= 4, "cap is half the stage-floor allocation");
        // Floor motes live in the lower band, above the ambient floor row.
        let floor_row = habitat.y + habitat.height - 1; // 11
        for g in &motes {
            assert!(g.row < floor_row, "motes never overwrite the floor row");
            assert!(g.row >= 7, "motes stay in the lower band");
        }
        // Exclusions are respected exactly like the ambient pass.
        let blocked = mote_glyphs_for(&day, habitat, &[habitat], now, ColorCapability::Truecolor);
        assert!(blocked.is_empty(), "fully excluded habitat places nothing");
    }

    #[test]
    fn mote_tidy_fade_thins_yesterdays_motes_after_rollover() {
        // 60x15 habitat: cells = 900, area_term = 11, stage-floor allocation
        // = 15, budget = floor(0.5*15) = 7. Yesterday ratio 3.0, today 0.0:
        // at +0min round(7*0.95) = 7 motes, at +15min round(7*0.95*0.5) = 3,
        // at +30min (the window edge) zero — no mid-grind vanish at 00:00.
        let habitat = Rect::new(0, 0, 60, 15);
        let day_start = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let day = crate::tui::day::DayContext {
            mature: true,
            today_ratio: 0.0,
            yesterday: Some(crate::tui::day::DaySummary {
                ratio: 3.0,
                dominant_shape: None,
            }),
            local_day_started_utc: day_start,
            date_seed: 7,
            ..crate::tui::day::DayContext::default()
        };
        let at = |minutes: i64| {
            mote_glyphs_for(
                &day,
                habitat,
                &[],
                day_start + time::Duration::minutes(minutes),
                ColorCapability::Truecolor,
            )
        };
        let t0 = at(0);
        let t15 = at(15);
        let t30 = at(30);
        assert!(!t0.is_empty(), "yesterday's motes are still in the room");
        assert!(!t15.is_empty(), "mid-window the fade is partial");
        assert!(
            t15.len() < t0.len(),
            "fade is monotonic: {} -> {}",
            t0.len(),
            t15.len()
        );
        assert!(t30.is_empty(), "tidy fade completes at the window edge");
        // The fading set holds still: the mid-window motes are a prefix of
        // the start-of-window set (same date_seed sequence, shorter count).
        for g in &t15 {
            assert!(t0.contains(g), "fade removes from the end, never reshuffles");
        }
    }

    #[test]
    fn flat_and_immature_pets_render_zero_motes() {
        let habitat = Rect::new(0, 0, 40, 12);
        let now = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let mature = crate::tui::day::DayContext {
            mature: true,
            today_ratio: 5.0,
            ..crate::tui::day::DayContext::default()
        };
        assert!(
            mote_glyphs_for(&mature, habitat, &[], now, ColorCapability::Flat).is_empty(),
            "Flat keeps the zero-ambient contract"
        );
        let immature = crate::tui::day::DayContext {
            mature: false,
            today_ratio: 5.0,
            ..crate::tui::day::DayContext::default()
        };
        assert!(
            mote_glyphs_for(&immature, habitat, &[], now, ColorCapability::Truecolor).is_empty(),
            "the default 100k baseline must not render a fabricated feast"
        );
    }
```

- [ ] **Step 2: Run tests, verify they fail to compile**

Run: `cargo test --lib mote 2>&1 | head -40`
Expected: compile errors — `cannot find function mote_glyphs_for`, `cannot find function mote_density`, and (unless the tiredness task already added it) `struct DayContext has no field named local_day_started_utc`.

- [ ] **Step 3: Implement**

In `src/tui/day.rs` — Read day.rs:30-36 first, then add after `PHASE_BLEND_MINUTES` (day.rs:35):

```rust
/// Motes fade out over this window after local-day rollover instead of
/// vanishing mid-grind at 00:00 (spec: Boundary behavior).
pub const MOTE_TIDY_FADE_MINUTES: i64 = 30;
```

Still in day.rs, the three `local_day_started_utc` edits (**skip if the field already exists** — see the dependency note). Read day.rs:196-224 and day.rs:353-373 before editing:

1. In the `DayContext` struct, after `pub local_day_rollover_utc: time::OffsetDateTime,` (day.rs:199):

```rust
    /// UTC instant the current local day began (motes tidy-fade anchor).
    pub local_day_started_utc: time::OffsetDateTime,
```

2. In `impl Default for DayContext`, after `local_day_rollover_utc: epoch,` (day.rs:221):

```rust
            local_day_started_utc: epoch,
```

3. In the `DayContext { ... }` literal at the bottom of `build_day_context`, after `local_day_rollover_utc: tomorrow_start,` (day.rs:371 — `today_start` is already in scope from day.rs:288):

```rust
            local_day_started_utc: today_start,
```

In `src/tui/panels/pet.rs` — Read pet.rs:29-32 first, then add after `const PET_H: u16 = 10;` (pet.rs:32):

```rust
/// Day-accumulation motes may use at most this share of the ambient glyph
/// allocation — the room never crowds the sky (spec: Day accumulation).
const MOTE_BUDGET_SHARE: f64 = 0.5;
/// Floor-mote glyphs: soft specks, deliberately sub-countable.
const MOTE_GLYPHS: &[char] = &['·', '.', ','];
```

Add the helpers after `ambient_glyphs_for_phase`'s closing brace (Read pet.rs:444-465 for the anchor; insert before `/// Returns extra work-activity glyphs...` at pet.rs:464):

```rust
/// Soft-saturating day-accumulation density in `today_ratio`: asymptotic and
/// sub-countable, so no learnable "full room" exists. No numbers, no fill
/// direction, no completion framing (spec: Day accumulation).
fn mote_density(ratio: f32) -> f32 {
    1.0 - (-ratio.max(0.0)).exp()
}

/// Day-accumulation floor motes. Density tracks `today_ratio` with soft
/// saturation, capped at MOTE_BUDGET_SHARE of the ambient allocation.
/// Placement is jittered by `date_seed` and stable all day — the room
/// accumulates instead of reshuffling, and a growing count extends the same
/// position sequence so existing motes hold still. For the first
/// MOTE_TIDY_FADE_MINUTES after the local day started, yesterday's density
/// fades to zero instead of vanishing at 00:00 (`date_seed` rolls at dawn,
/// not midnight, so the fading set keeps last night's positions). Flat
/// renders zero motes (ambient contract unchanged); immature pets render
/// zero (spec: Maturity gate governs every baseline-ratio channel).
fn mote_glyphs_for(
    day: &crate::tui::day::DayContext,
    habitat: Rect,
    exclusions: &[Rect],
    now: time::OffsetDateTime,
    color_capability: crate::tui::style::ColorCapability,
) -> Vec<AmbientGlyph> {
    if matches!(color_capability, ColorCapability::Flat) || !day.mature {
        return Vec::new();
    }
    // Need a band of at least one mote row above the ambient floor row.
    if habitat.width == 0 || habitat.height < 3 {
        return Vec::new();
    }

    // Budget: MOTE_BUDGET_SHARE of the ambient allocation. The binding
    // signature carries no stage, so the budget uses the allocation formula's
    // stage floor (stage_base_count's minimum, 4) — guaranteeing the cap
    // stays at or under MOTE_BUDGET_SHARE of every stage's allocation.
    let habitat_cells = (habitat.width as usize) * (habitat.height as usize);
    let area_term = habitat_cells.saturating_sub(200) / 60;
    let allocation_floor = (4 + area_term) as f64 * phase_count_scale(day.day_phase);
    let budget = (MOTE_BUDGET_SHARE * allocation_floor).floor();

    let today_count = (budget * f64::from(mote_density(day.today_ratio))).round() as usize;

    // Tidy fade: yesterday's motes thin out over MOTE_TIDY_FADE_MINUTES
    // after the local day starts (spec: Boundary behavior).
    let fade_elapsed = (now - day.local_day_started_utc).whole_seconds() as f32;
    let fade_window = (crate::tui::day::MOTE_TIDY_FADE_MINUTES as f32) * 60.0;
    let fading_count = match day.yesterday {
        Some(y) if fade_elapsed >= 0.0 && fade_elapsed < fade_window => {
            let remaining = 1.0 - fade_elapsed / fade_window;
            (budget * f64::from(mote_density(y.ratio) * remaining)).round() as usize
        }
        _ => 0,
    };
    let count = today_count.max(fading_count);
    if count == 0 {
        return Vec::new();
    }

    let mut rng = Pcg32::seed_from_u64(day.date_seed.wrapping_mul(0xA076_1D64_78BD_642F));
    let p = crate::tui::style::tokenpet_palette();
    let color = warm_shift(p.dim.rgb, 0.15);
    // Lower band of the habitat, above the floor row the ambient pass owns.
    let band = (habitat.height / 3)
        .max(1)
        .min(habitat.height.saturating_sub(2));
    let band_top = habitat.y + habitat.height - 1 - band;
    let mut glyphs: Vec<AmbientGlyph> = Vec::with_capacity(count);
    for _ in 0..count {
        for _attempt in 0..16 {
            let col = habitat.x + rng.gen_range(0..habitat.width);
            let row = band_top + rng.gen_range(0..band);
            let candidate = AmbientGlyph {
                row,
                col,
                glyph: *MOTE_GLYPHS.choose(&mut rng).unwrap_or(&'·'),
                color,
            };
            if !overlaps_any(&candidate, exclusions)
                && !glyphs
                    .iter()
                    .any(|g| g.col == candidate.col && g.row == candidate.row)
            {
                glyphs.push(candidate);
                break;
            }
        }
    }
    glyphs
}
```

Wire the render pass. Read pet.rs:653-660 first; between the ambient paint loop's closing brace (pet.rs:659) and `let compact = area.width <= 72 || area.height <= 24;` (pet.rs:660), insert:

```rust
        // Mote pass: after ambient, before activity glyphs, same exclusions
        // (silhouette halo + speech) — spec: Day accumulation.
        let motes = mote_glyphs_for(
            &vm.day_context,
            scene.habitat,
            &ambient_exclusions,
            now,
            ctx.color_capability,
        );
        for g in motes {
            if ambient_glyph_is_inside_area(&g, scene.habitat) {
                let cell = &mut buf[(g.col, g.row)];
                cell.set_char(g.glyph);
                cell.set_style(Style::default().fg(g.color));
            }
        }
```

(The draw-order guard in `tests/tui_render.rs:1243-1261` keys on `ambient_glyphs_for_phase(` < `habitat_props_for(` < `render_pet_inside(` — this insertion preserves that order.)

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test --lib mote -- --nocapture && cargo test --lib day_context_carries`
Expected: 4 passed (mote_density / motes_cap / mote_tidy_fade / flat_and_immature), then 1 passed.

- [ ] **Step 5: Full suite + preview snapshots**

Run: `cargo test 2>&1 | tail -20`
Expected: green, with one possible exception: `tests/dev_preview.rs` whole-frame insta snapshots (`dev_preview__watch_wide_normal_frame.snap`) MAY fail if the seeded watch fixture's ledger is mature and `today_ratio > 0` — the new mote layer is then real content landing in the frame. If it fails: inspect the diff with `cargo insta review` (or read the generated `.snap.new`), confirm the only change is new `·`/`.`/`,` specks in the lower habitat band, accept, and include `tests/snapshots/` in the commit. Any other frame difference is a defect — stop and debug. The `watch-daycontext-*` fixtures override `day_context` with `mature: false` defaults (src/dev_preview/watch.rs:591-634), so their snapshots must NOT change.

- [ ] **Step 6: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add src/tui/day.rs src/tui/panels/pet.rs tests/snapshots
git commit -m "feat(tui): add day-accumulation floor motes with soft saturation and tidy fade"
```

---

### Task 8: Today's sky character + climate tint + season drift

**Spec sections:** "Branch T3 — Today's sky character" (`date_seed` picks the day's sky glyph family variant; visual texture only — locked rule), "Climate rendering" (`None`/`Clear` render nothing), "Seasons" (subtle hue drift only, never named in any text).

**Interface sheet bindings:** extend the EXISTING T1 functions in `src/tui/panels/pet.rs` — do not duplicate them:
`fn sky_palette_for_phase(species: Species, phase: DayPhase, date_seed: u64) -> &'static [char]` (≥2 authored variants per (species, phase)) and
`fn sky_color_for_phase(phase: DayPhase, blend: f32, season: Season, climate: Option<WorkWeather>) -> Color`.
`ambient_glyphs_for_phase` already carries `#[allow(clippy::too_many_arguments)]` (pet.rs:376) — adding `date_seed`/`season`/`climate` params continues that precedent.

**Files:**
- Modify: `src/tui/panels/pet.rs` (import at pet.rs:20; `sky_palette_for_phase` at pet.rs:225-237; `sky_color_for_phase` at pet.rs:249-263; `ambient_glyphs_for` wrapper at pet.rs:351-369; `ambient_glyphs_for_phase` at pet.rs:376-412; production call site at pet.rs:643-652; existing tests at pet.rs:2052-2141)
- Test: in-module tests at the bottom of `pet.rs`

- [ ] **Step 1: Write the failing tests**

Append to the `pet.rs` tests mod:

```rust
    #[test]
    fn sky_family_is_stable_for_a_seed_and_authors_two_variants_per_phase() {
        let all_species = [
            Species::Fuzz,
            Species::Blob,
            Species::Ghost,
            Species::Glitch,
            Species::Crystal,
            Species::Mech,
        ];
        let phases = [
            DayPhase::Dawn,
            DayPhase::Day,
            DayPhase::Dusk,
            DayPhase::Night,
        ];
        for species in all_species {
            for phase in phases {
                // Same date_seed -> same family, every call.
                assert_eq!(
                    sky_palette_for_phase(species, phase, 9),
                    sky_palette_for_phase(species, phase, 9),
                    "{species:?}/{phase:?} family must be a pure function of the seed"
                );
                // >=2 authored variants: consecutive seeds pick different
                // families for every (species, phase).
                assert_ne!(
                    sky_palette_for_phase(species, phase, 8),
                    sky_palette_for_phase(species, phase, 9),
                    "{species:?}/{phase:?} needs at least two authored variants"
                );
            }
        }
    }

    #[test]
    fn climate_clear_and_none_tint_nothing_and_a_real_climate_tints() {
        for phase in [
            DayPhase::Dawn,
            DayPhase::Day,
            DayPhase::Dusk,
            DayPhase::Night,
        ] {
            assert_eq!(
                sky_color_for_phase(phase, 1.0, Season::Summer, None),
                sky_color_for_phase(phase, 1.0, Season::Summer, Some(WorkWeather::Clear)),
                "Clear must render exactly like None for {phase:?}"
            );
        }
        assert_ne!(
            sky_color_for_phase(DayPhase::Day, 1.0, Season::Summer, None),
            sky_color_for_phase(DayPhase::Day, 1.0, Season::Summer, Some(WorkWeather::CacheMist)),
            "a real climate biases the ambient tint"
        );
    }

    #[test]
    fn season_drift_is_bounded_and_summer_is_the_neutral_reference() {
        let c = Color::Rgb(110, 110, 110);
        assert_eq!(season_hue_drift(c, Season::Summer), c);
        for season in [
            crate::tui::day::Season::Spring,
            crate::tui::day::Season::Autumn,
            crate::tui::day::Season::Winter,
        ] {
            let drifted = season_hue_drift(c, season);
            assert_ne!(drifted, c, "{season:?} must drift the hue");
            let Color::Rgb(r, g, b) = drifted else {
                panic!("rgb in, rgb out");
            };
            for channel in [r, g, b] {
                assert!(
                    (i16::from(channel) - 110).abs()
                        <= i16::from(SEASON_DRIFT_MAX_CHANNEL_NUDGE),
                    "{season:?} drift must stay subtle (channel {channel})"
                );
            }
        }
        // Non-RGB colors (terminal-capability fallbacks) pass through.
        assert_eq!(season_hue_drift(Color::Reset, Season::Winter), Color::Reset);
    }
```

Also update the three existing tests to the new signatures (Read pet.rs:2052-2141 first):
- `night_sky_uses_the_night_family_and_a_smaller_budget` (pet.rs:2053): both `ambient_glyphs_for_phase(...)` calls gain `0, Season::Summer, None,` after the `1.0,` argument.
- `flat_tier_still_renders_zero_ambient_glyphs_at_night` (pet.rs:2099): same three extra arguments.
- `phase_blend_interpolates_the_sky_color` (pet.rs:2119): the three calls become `sky_color_for_phase(phase, 0.0, Season::Summer, None)`, `sky_color_for_phase(phase, 1.0, Season::Summer, None)`, `sky_color_for_phase(phase, 0.5, Season::Summer, None)` — its assertions are unchanged because Summer + None are the neutral identity.

- [ ] **Step 2: Run tests, verify they fail to compile**

Run: `cargo test --lib sky_family 2>&1 | head -40`
Expected: compile errors — `sky_palette_for_phase` takes 2 arguments but 3 were supplied, `sky_color_for_phase` takes 2 arguments but 4 were supplied, `cannot find function season_hue_drift`, `Season` not found in this scope.

- [ ] **Step 3: Implement**

Change the day import (pet.rs:20) from `use crate::tui::day::DayPhase;` to:

```rust
use crate::tui::day::{DayPhase, Season};
```

Replace `sky_palette_for_phase` (Read pet.rs:223-237 first) with:

```rust
/// Per-phase sky glyph family, with `date_seed` picking among authored
/// variants per (species, phase) — the day's character is visual texture
/// only, never personality content (locked rule). Night stays a sparse
/// starfield, dawn/dusk warm grain, day a species family.
fn sky_palette_for_phase(species: Species, phase: DayPhase, date_seed: u64) -> &'static [char] {
    let variant = (date_seed % 2) as usize;
    match phase {
        DayPhase::Day => {
            if variant == 0 {
                sky_palette_for(species)
            } else {
                match species {
                    Species::Fuzz => &['*', '·', '`', '.'],
                    Species::Blob => &['o', '·', '°', '.'],
                    Species::Ghost => &['\'', '~', '·', ','],
                    Species::Glitch => &['░', '▒', '▪', '·'],
                    Species::Crystal => &['✧', '·', '✦', '◇'],
                    Species::Mech => &['°', '·', '─', '○'],
                }
            }
        }
        DayPhase::Dawn | DayPhase::Dusk => {
            let variants: [&'static [char]; 2] = match species {
                Species::Glitch => [&['░', '▪', '·', ' '], &['·', '░', '▪', ' ']],
                _ => [&['·', '\'', '~', ' '], &['\'', ',', '·', ' ']],
            };
            variants[variant]
        }
        DayPhase::Night => {
            let variants: [&'static [char]; 2] = match species {
                Species::Glitch => [&['▪', '·', ' ', ' '], &['·', '▪', '.', ' ']],
                _ => [&['✦', '·', '*', ' '], &['*', '·', '✧', ' ']],
            };
            variants[variant]
        }
    }
}
```

(`sky_palette_for` at pet.rs:70-79 stays — it is Day variant 0 and is referenced by the existing night-family test.)

Replace `sky_color_for_phase` (Read pet.rs:249-263 first) with the extended version plus its two helpers:

```rust
/// Bounded seasonal hue drift on the sky color. Summer is the neutral
/// reference; the other seasons nudge channels by at most
/// SEASON_DRIFT_MAX_CHANNEL_NUDGE. Drift only — the season is never named in
/// any UI text, speech, or dream (spec: Seasons).
const SEASON_DRIFT_MAX_CHANNEL_NUDGE: u8 = 8;

fn season_hue_drift(color: Color, season: Season) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    match season {
        Season::Summer => color,
        Season::Spring => Color::Rgb(r, g.saturating_add(6), b),
        Season::Autumn => Color::Rgb(r.saturating_add(7), g, b.saturating_sub(5)),
        Season::Winter => Color::Rgb(r.saturating_sub(4), g, b.saturating_add(7)),
    }
}

/// Ambient tint bias from the 7-day climate class. None and Clear both
/// render nothing (spec: Climate rendering); the class -> color mapping
/// matches the live weather channel's activity_glyph_color so the two
/// channels never disagree about what a class looks like.
const CLIMATE_TINT_WEIGHT: f32 = 0.12;

fn climate_tint(color: Color, climate: Option<WorkWeather>) -> Color {
    let p = crate::tui::style::tokenpet_palette();
    let target = match climate {
        None | Some(WorkWeather::Clear) => return color,
        Some(WorkWeather::CacheMist) => p.good.rgb,
        Some(WorkWeather::OutputSparks) => p.accent.rgb,
        Some(WorkWeather::ReasoningPulse) => p.bad.rgb,
        Some(WorkWeather::Mixed) => p.good.rgb,
    };
    lerp_color(color, target, CLIMATE_TINT_WEIGHT)
}

/// Sky color for `phase`, interpolated from the neutral dim base toward the
/// phase's target warmth/dim over `blend` (0.0 at the boundary, 1.0 after
/// PHASE_BLEND_MINUTES), then drifted by season and biased by climate.
/// Summer + None/Clear is the neutral identity.
fn sky_color_for_phase(
    phase: DayPhase,
    blend: f32,
    season: Season,
    climate: Option<WorkWeather>,
) -> Color {
    let p = crate::tui::style::tokenpet_palette();
    let base = p.dim.rgb;
    let target = match phase {
        DayPhase::Day => base,
        DayPhase::Dawn => warm_shift(base, 0.25),
        DayPhase::Dusk => warm_shift(base, 0.40),
        DayPhase::Night => dim_shift(base, 0.40),
    };
    climate_tint(season_hue_drift(lerp_color(base, target, blend), season), climate)
}
```

Extend `ambient_glyphs_for_phase` (Read pet.rs:376-412 first). The signature gains three params after `phase_blend`:

```rust
#[allow(clippy::too_many_arguments)]
pub fn ambient_glyphs_for_phase(
    species: Species,
    stage: Stage,
    habitat: Rect,
    exclusions: &[Rect],
    now: time::OffsetDateTime,
    color_capability: ColorCapability,
    phase: DayPhase,
    phase_blend: f32,
    date_seed: u64,
    season: Season,
    climate: Option<WorkWeather>,
) -> Vec<AmbientGlyph> {
```

and the two body lines change (pet.rs:407 and pet.rs:412):

```rust
    let sky = sky_palette_for_phase(species, phase, date_seed);
```

```rust
    let sky_color = sky_color_for_phase(phase, phase_blend, season, climate);
```

(The floor row keeps its existing neutral/night-dim color — climate and season bias the sky only.)

Update the day-only wrapper `ambient_glyphs_for` (Read pet.rs:351-369 first) to delegate with the neutral identities:

```rust
    ambient_glyphs_for_phase(
        species,
        stage,
        habitat,
        exclusions,
        now,
        color_capability,
        DayPhase::Day,
        1.0,
        0,
        Season::Summer,
        None,
    )
```

Update the production render call site (Read pet.rs:643-652 first):

```rust
        let glyphs = ambient_glyphs_for_phase(
            species,
            stage,
            scene.habitat,
            &ambient_exclusions,
            now,
            ctx.color_capability,
            vm.day_context.day_phase,
            phase_blend,
            vm.day_context.date_seed,
            vm.day_context.season,
            vm.day_context.climate,
        );
```

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test --lib sky_family && cargo test --lib climate_clear && cargo test --lib season_drift && cargo test --lib phase_blend && cargo test --lib panels::pet`
Expected: 1, 1, 1, 1 passed, then the whole pet-panel module green (including the updated night-family and Flat tests).

- [ ] **Step 5: Full suite + preview snapshots**

Run: `cargo test 2>&1 | tail -20`
Expected: green, except `dev_preview__watch_wide_normal_frame.snap` MAY fail — that fixture's `date_seed` is a real fnv hash of (dawn-rolled date, pet seed) and may be odd, flipping its Day sky family to variant 1. If it fails: inspect with `cargo insta review`, confirm the only diff is sky glyph substitutions (family characters, same positions/budget), accept, and include `tests/snapshots/` in the commit. The `watch-daycontext-night-asleep` snapshot must NOT change (its override context has `date_seed: 0` → variant 0, `Season::Summer` + `climate: None` → neutral identity). Color-only effects (season drift, climate tint) never touch `frames/*.txt` snapshots — they carry visible cells only.

- [ ] **Step 6: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add src/tui/panels/pet.rs tests/snapshots
git commit -m "feat(tui): date-seeded sky families with climate tint and season drift"
```

---

### Task 9: Weekend texture — softening for wander cadence and scene palette

**Spec section:** "Branch T3 — Weekend texture" (softer palette and lazier wander cadence when `is_weekend`, scaled by ledger-derived `weekend_share`; full softening at share ≤ `WEEKEND_QUIET_SHARE` (0.10), zero at ≥ `WEEKEND_ACTIVE_SHARE` (0.30); a weekend-active user gets no sleepy Saturday room; live-activity channels always win; maturity-gated).

**Interface sheet bindings:** `pub fn weekend_softening(day: &DayContext) -> f32` in `src/tui/day.rs`; `WEEKEND_QUIET_SHARE = 0.10` and `WEEKEND_ACTIVE_SHARE = 0.30` in day.rs; consumed as wander-cadence + palette-warmth scaling with live-activity channels always winning.

**Files:**
- Modify: `src/tui/day.rs` (constants after `MOTE_TIDY_FADE_MINUTES` from Task 7; `weekend_softening` after `scene_asleep_for_poll` at day.rs:376-383)
- Modify: `src/pet/animator.rs` (`lazy_wander_instant` after `compute_wake_wander_x` ends at animator.rs:436)
- Modify: `src/tui/panels/pet.rs` (animator import at pet.rs:12-16; `effective_weekend_softening` + `weekend_soften_color` helpers; wander match at pet.rs:586-606; ambient + mote paint loops)
- Test: in-module tests in all three files

- [ ] **Step 1: Write the failing tests**

In `src/tui/day.rs` tests mod:

```rust
    #[test]
    fn weekend_softening_maps_share_boundaries_and_respects_the_gates() {
        let ctx = |share: f32, mature: bool, is_weekend: bool| DayContext {
            is_weekend,
            mature,
            weekend_share: share,
            ..DayContext::default()
        };
        // Full softening at or below the quiet share.
        assert_eq!(weekend_softening(&ctx(0.05, true, true)), 1.0);
        assert_eq!(weekend_softening(&ctx(WEEKEND_QUIET_SHARE, true, true)), 1.0);
        // Zero at or above the active share — a weekend-active user gets no
        // sleepy Saturday room.
        assert_eq!(weekend_softening(&ctx(WEEKEND_ACTIVE_SHARE, true, true)), 0.0);
        assert_eq!(weekend_softening(&ctx(0.45, true, true)), 0.0);
        // Linear in between.
        let mid = weekend_softening(&ctx(0.20, true, true));
        assert!((mid - 0.5).abs() < 1e-5, "expected ~0.5, got {mid}");
        // Weekdays never soften, and the maturity gate governs this channel
        // like every baseline-ratio channel.
        assert_eq!(weekend_softening(&ctx(0.05, true, false)), 0.0);
        assert_eq!(weekend_softening(&ctx(0.05, false, true)), 0.0);
    }
```

In `src/pet/animator.rs` tests mod (starts animator.rs:564):

```rust
    #[test]
    fn lazy_wander_instant_dilates_elapsed_time_only_when_softened() {
        let anchor = time::OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap();
        let now = anchor + time::Duration::minutes(60);
        // Zero softening: the wander clock is the wall clock.
        assert_eq!(lazy_wander_instant(now, anchor, 0.0), now);
        // Full softening: the wander clock runs at half speed — 60 elapsed
        // minutes read as 30 (cadence multiplied by 2).
        assert_eq!(
            lazy_wander_instant(now, anchor, 1.0),
            anchor + time::Duration::minutes(30)
        );
        // A pre-anchor instant (clock skew) clamps to the anchor, no panic.
        assert_eq!(
            lazy_wander_instant(anchor - time::Duration::minutes(5), anchor, 1.0),
            anchor
        );
    }
```

In `src/tui/panels/pet.rs` tests mod:

```rust
    #[test]
    fn live_activity_always_wins_over_weekend_softening() {
        let day = crate::tui::day::DayContext {
            is_weekend: true,
            mature: true,
            weekend_share: 0.05,
            ..crate::tui::day::DayContext::default()
        };
        let idle = PetLifeProfile::idle();
        assert!(
            (effective_weekend_softening(&day, &idle) - 1.0).abs() < 1e-6,
            "quiet weekend, idle pet: full softening"
        );
        let mut active = PetLifeProfile::idle();
        active.activity_level = 0.8;
        assert_eq!(
            effective_weekend_softening(&day, &active),
            0.0,
            "live activity suppresses softening entirely"
        );
        let mut bursting = PetLifeProfile::idle();
        bursting.burst_level = 0.4;
        assert_eq!(
            effective_weekend_softening(&day, &bursting),
            0.0,
            "a live burst suppresses softening entirely"
        );
    }

    #[test]
    fn weekend_softening_pulls_scene_colors_toward_the_dim_base() {
        let c = Color::Rgb(200, 120, 40);
        assert_eq!(weekend_soften_color(c, 0.0), c, "no softening, no change");
        assert_ne!(weekend_soften_color(c, 1.0), c, "full softening shifts the color");
        // Non-RGB colors pass through untouched.
        assert_eq!(weekend_soften_color(Color::Reset, 1.0), Color::Reset);
    }
```

- [ ] **Step 2: Run tests, verify they fail to compile**

Run: `cargo test --lib weekend 2>&1 | head -40`
Expected: compile errors — `cannot find function weekend_softening`, `cannot find value WEEKEND_QUIET_SHARE`, `cannot find function effective_weekend_softening`, `cannot find function weekend_soften_color`; `cargo test --lib lazy_wander 2>&1 | head -10` likewise fails with `cannot find function lazy_wander_instant`.

- [ ] **Step 3: Implement**

In `src/tui/day.rs`, after `MOTE_TIDY_FADE_MINUTES` (added in Task 7 — Read the constants block at day.rs:17-38 first):

```rust
/// Weekend softening is full when the ledger's weekend share of window
/// volume is at or below this...
pub const WEEKEND_QUIET_SHARE: f32 = 0.10;
/// ...and zero at or above this — a weekend-active user gets no sleepy
/// Saturday room (spec: Weekend texture).
pub const WEEKEND_ACTIVE_SHARE: f32 = 0.30;
```

After `scene_asleep_for_poll`'s closing brace (Read day.rs:375-384 first):

```rust
/// Weekend softening factor 0.0 (none) ..= 1.0 (full), from is_weekend and
/// weekend_share: full at share <= WEEKEND_QUIET_SHARE, zero at
/// >= WEEKEND_ACTIVE_SHARE, linear between; 0.0 while immature (spec: the
/// Maturity gate governs every baseline-ratio channel) and on weekdays.
pub fn weekend_softening(day: &DayContext) -> f32 {
    if !day.is_weekend || !day.mature {
        return 0.0;
    }
    if day.weekend_share <= WEEKEND_QUIET_SHARE {
        1.0
    } else if day.weekend_share >= WEEKEND_ACTIVE_SHARE {
        0.0
    } else {
        (WEEKEND_ACTIVE_SHARE - day.weekend_share) / (WEEKEND_ACTIVE_SHARE - WEEKEND_QUIET_SHARE)
    }
}
```

In `src/pet/animator.rs`, after `compute_wake_wander_x`'s closing brace (Read animator.rs:425-440 first):

```rust
/// Weekend-lazy wander clock: runs at 1/(1 + softening) speed, anchored at a
/// vm-carried instant so motion is continuous while softening holds. A
/// softening change (live activity beginning, the midnight weekend edge)
/// re-times the clock in one step — bounded, poll-aligned, and coincident
/// with the activity that caused it (live channels win; spec: Weekend
/// texture).
pub fn lazy_wander_instant(
    now: time::OffsetDateTime,
    anchor: time::OffsetDateTime,
    softening: f32,
) -> time::OffsetDateTime {
    let s = softening.clamp(0.0, 1.0);
    if s <= 0.0 {
        return now;
    }
    let elapsed = (now - anchor).as_seconds_f64().max(0.0);
    anchor + time::Duration::seconds_f64(elapsed / (1.0 + f64::from(s)))
}
```

In `src/tui/panels/pet.rs`:

1. Add `lazy_wander_instant` to the animator import (Read pet.rs:12-16 first):

```rust
use crate::pet::animator::{
    compute_facing, compute_shimmer_role, compute_sleep_wander_x, compute_token_pop,
    compute_twinkle, compute_wake_wander_x, compute_wander_position_x, lazy_wander_instant,
    low_energy_lightness_multiplier, TokenPop,
};
```

2. Add the two helpers after `mote_glyphs_for`'s closing brace (from Task 7):

```rust
/// Live-activity channels always win over weekend softening: any live
/// signal suppresses it entirely (spec: Weekend texture).
fn effective_weekend_softening(
    day: &crate::tui::day::DayContext,
    profile: &PetLifeProfile,
) -> f32 {
    if profile.burst_level > 0.0 || profile.activity_level > 0.0 {
        return 0.0;
    }
    crate::tui::day::weekend_softening(day)
}

/// Weekend palette softening: pulls a scene color toward the neutral dim
/// base. Applied to the ambient and mote passes only — activity glyphs and
/// the pet itself are live channels and stay untouched.
const WEEKEND_PALETTE_SOFTEN_MAX: f32 = 0.25;

fn weekend_soften_color(color: Color, softening: f32) -> Color {
    if softening <= 0.0 {
        return color;
    }
    let p = crate::tui::style::tokenpet_palette();
    lerp_color(color, p.dim.rgb, WEEKEND_PALETTE_SOFTEN_MAX * softening.clamp(0.0, 1.0))
}
```

3. Wire the wander cadence. Read pet.rs:584-618 first. After `let day = &vm.day_context;` insert the softening computation, and replace the catch-all match arm:

```rust
        let day = &vm.day_context;
        let softening = effective_weekend_softening(day, &vm.life_profile);
        let (wander_x, facing) = match (day.asleep, day.sleep_onset_utc, day.wake_resume) {
            (true, Some(onset), _) => (
                compute_sleep_wander_x(area.width, species, now, onset),
                compute_facing(area.width, species, onset), // held facing: no mirror flips with shut eyes
            ),
            (false, _, Some(resume)) => (
                compute_wake_wander_x(
                    area.width,
                    species,
                    now,
                    resume.from_eval_utc,
                    resume.woke_at_utc,
                ),
                compute_facing(area.width, species, now),
            ),
            _ => {
                // Weekend-lazy cadence: the wander clock slows by
                // 1/(1 + softening), anchored at the carried day start so
                // position and facing stay mutually consistent.
                let wander_now =
                    lazy_wander_instant(now, day.local_day_started_utc, softening);
                (
                    compute_wander_position_x(area.width, species, wander_now),
                    compute_facing(area.width, species, wander_now),
                )
            }
        };
```

4. Wire the palette warmth. In the ambient paint loop (Read pet.rs:653-659 first — line numbers shifted by the Task 7 insertion; anchor on the code text), change:

```rust
                cell.set_style(ratatui::style::Style::default().fg(g.color));
```

to:

```rust
                cell.set_style(
                    ratatui::style::Style::default().fg(weekend_soften_color(g.color, softening)),
                );
```

and in the mote paint loop (from Task 7), change `cell.set_style(Style::default().fg(g.color));` to:

```rust
                cell.set_style(Style::default().fg(weekend_soften_color(g.color, softening)));
```

The activity-glyph paint loop is NOT touched — live channels win.

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test --lib weekend -- --nocapture && cargo test --lib lazy_wander`
Expected: 3 passed (weekend_softening_maps... in day.rs, live_activity_always_wins... and weekend_softening_pulls... in pet.rs), then 1 passed.

- [ ] **Step 5: Full suite**

Run: `cargo test 2>&1 | tail -20`
Expected: green, including the dev_preview snapshots unchanged — the preview clock `1_760_000_000` (src/dev_preview/scenarios.rs:41) is a Thursday, so `is_weekend` is false in every fixture and both the lazy wander clock and the palette softening are identities there. If any snapshot changes in this task, that is a defect — stop and debug.

- [ ] **Step 6: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add src/tui/day.rs src/pet/animator.rs src/tui/panels/pet.rs
git commit -m "feat(tui): weekend softening for wander cadence and scene palette"
```
<!-- PART D: Tasks 10-12 (resonance + proof). Written against the post-T1 tree
     plus the assumption that Tasks 1-9 (guard, local timestamps, tiredness,
     motes, morning-after, dreams + speech precedence, sky character, climate +
     seasons, weekend softening) have landed. DayContext therefore already
     carries `tiredness: f32` and `local_day_started_utc` (interface sheet,
     "DayContext additions"), and `current_pet_speech_for_scene` already has the
     4-arg `&DayContext` signature. Anchor warning: every line number cited
     below is from the post-T1 extraction contracts — Tasks 1-9 will have
     shifted them. ALWAYS Read the target region before editing. -->

### Task 10a (stitcher addendum): yesterday's source mix on DayContext

**Why this task exists:** the spec's "codex lamp after a codex-heavy day" needs
yesterday's claude/codex split, which `DayContext` did not carry — without it
the lamp arm degenerates to a once-ever fresh-unlock match. One applied
per-source query + one derived field fixes it. Run this BEFORE Task 10.

**Files:**
- Modify: `src/storage/usage_store.rs` (new applied per-source aggregate)
- Modify: `src/tui/day.rs` (`yesterday_codex_share` field + `CODEX_HEAVY_SHARE`)
- Modify: `src/tui/view_model.rs` (fixture default, if the fixture spells out fields)
- Test: in-module tests in both files

- [ ] **Step 1: Write the failing tests**

```rust
    // usage_store.rs tests
    #[test]
    fn applied_effective_tokens_by_source_between_groups_applied_rows_per_surface() {
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let mut codex = sample_event_at(now, 3_000.0);
        codex.provider_surface = "codex".into();
        store.insert_event(&codex).unwrap();
        store.insert_event(&sample_event_at(now, 1_000.0)).unwrap(); // claude-code
        let totals = store
            .applied_effective_tokens_by_source_between(
                now - time::Duration::hours(1),
                now + time::Duration::seconds(1),
            )
            .unwrap();
        assert_eq!(totals.len(), 2);
        assert!(totals.contains(&("codex".to_string(), 3_000.0)));
    }
```

```rust
    // day.rs tests
    #[test]
    fn yesterday_codex_share_reflects_the_applied_source_mix() {
        let now = datetime!(2026-06-09 12:00 UTC);
        let yesterday = now - time::Duration::days(1);
        let mut store = UsageStore::open(":memory:".as_ref()).unwrap();
        let mut codex = NormalizedUsageEvent {
            observed_at: yesterday,
            bucket_at: yesterday,
            ..NormalizedUsageEvent::for_test_at(yesterday, 8_000.0)
        };
        codex.provider_surface = "codex".into();
        store.insert_event(&codex).unwrap();
        store
            .insert_event(&NormalizedUsageEvent {
                observed_at: yesterday,
                bucket_at: yesterday,
                ..NormalizedUsageEvent::for_test_at(yesterday, 2_000.0)
            })
            .unwrap(); // claude-code
        let mut state = crate::storage::state::PetState::new_for_test("seed", "buddy");
        state.created_at = now - time::Duration::days(3);
        let ctx = build_day_context(&store, &state, now, utc_mapper());
        assert!((ctx.yesterday_codex_share - 0.8).abs() < 1e-6);
        // No coverage / empty yesterday -> 0.0, never NaN.
        let empty = UsageStore::open(":memory:".as_ref()).unwrap();
        let ctx2 = build_day_context(&empty, &state, now, utc_mapper());
        assert_eq!(ctx2.yesterday_codex_share, 0.0);
    }
```

- [ ] **Step 2: Run, verify compile failures**

Run: `cargo test --lib applied_effective_tokens_by_source 2>&1 | head -10`

- [ ] **Step 3: Implement**

Store method (next to `applied_effective_tokens_between` — match its style):

```rust
    /// Applied-only per-source effective sums over the half-open bucket_at
    /// window `[start, end)`. DayContext's yesterday source mix; the
    /// unfiltered variant (`token_totals_by_source_between`) serves the
    /// today panel and must not change.
    pub fn applied_effective_tokens_by_source_between(
        &self,
        start: OffsetDateTime,
        end: OffsetDateTime,
    ) -> crate::error::Result<Vec<(String, f64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT provider_surface, COALESCE(SUM(effective_tokens), 0.0)
             FROM usage_events
             WHERE applied_at IS NOT NULL AND bucket_at >= ?1 AND bucket_at < ?2
             GROUP BY provider_surface
             ORDER BY provider_surface",
        )?;
        let rows = stmt
            .query_map(params![format_time(start)?, format_time(end)?], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
```

day.rs: add the constant next to `FEAST_DAY_RATIO`:

```rust
/// Yesterday counts as codex-heavy at or above this share of applied
/// effective tokens (mirrors classify_source_accent's codex-dominant edge).
pub const CODEX_HEAVY_SHARE: f32 = 0.6;
```

Add the field to `DayContext` (after `yesterday`):

```rust
    /// Codex share of yesterday's applied effective tokens, 0.0..=1.0.
    /// 0.0 when yesterday is None or had no applied volume.
    pub yesterday_codex_share: f32,
```

In `build_day_context`, alongside the existing yesterday derivation (Read the
region first — Task 4 reshaped it), inside the `Some` branch:

```rust
        let per_source = usage_store
            .applied_effective_tokens_by_source_between(y_start, today_start)
            .unwrap_or_default();
        let total: f64 = per_source.iter().map(|(_, v)| *v).sum();
        let codex: f64 = per_source
            .iter()
            .filter(|(name, _)| name == "codex")
            .map(|(_, v)| *v)
            .sum();
        let yesterday_codex_share = if total > 0.0 { (codex / total) as f32 } else { 0.0 };
```

(`yesterday_codex_share` is `0.0` on the `None` branch.) Add the field to the
struct literal, `Default` impl (`0.0`), and any spelled-out fixture
constructors the compiler flags.

- [ ] **Step 4: Run, verify pass**

Run: `cargo test --lib tui::day && cargo test --lib usage_store`

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features -- -D warnings
git add -u
git commit -m "feat(tui): carry yesterday's codex share on DayContext"
```

---

### Task 10: Prop resonance — yesterday's character picks a companion prop

**Spec section:** Branch T3 "Prop resonance" (spec:407-415), "Prop-resonance styling pauses while asleep" (spec:269), maturity gate for ratio-qualified resonance (spec:104-108).

**Interface (binding, from the sheet):**

```rust
pub fn resonant_prop_for_day(
    day: &DayContext,
    earned: &[crate::storage::state::EarnedHabitatProp],
) -> Option<crate::game::habitat::HabitatPropId>;
```

**Design notes (read before coding):**

- `HabitatPropId` is defined in `src/storage/state.rs:22` and only privately imported by `src/game/habitat.rs:6`. The sheet's return path `crate::game::habitat::HabitatPropId` requires a one-line `pub use` re-export in habitat.rs (added below) — same type, second path.
- Provenance matching (`HabitatPropSource`, src/storage/state.rs:47-54):
  - `HeavySession` → recurring day-character match: qualifies when **yesterday was a feast** (`yesterday.ratio >= FEAST_DAY_RATIO`). Baseline-ratio channel → **requires `day.mature`** (sheet: "Ratio-qualified matches require day.mature").
  - `ProviderFirstUse { .. }` (codex lamp) → **recurring source-mix match** (stitcher addendum, Task 10a): qualifies when `day.yesterday_codex_share >= CODEX_HEAVY_SHARE` (0.6, mirroring `classify_source_accent`'s codex-dominant boundary). Source-mix-qualified, not baseline-ratio-scaled → no maturity gate. (The original draft used fresh-unlock matching because `DayContext` carried no per-source dimension; Task 10a adds `yesterday_codex_share` precisely so the spec's "codex lamp after a codex-heavy day" stays a recurring charm rather than a once-ever event.)
  - `WiltRecovery` (recovery sprout) → one-shot event provenance: qualifies while the unlock story is **fresh** — `earned_at >= day.local_day_started_utc - 1 day` (recovered yesterday or today, judged against the carried instant; the fixed 24h subtraction is at most an hour off across DST, cosmetically irrelevant). Event-qualified → no maturity gate.
  - `LifetimeTokens { .. }` → a ladder milestone, not a day character. Never resonates.
- `date_seed` tie-breaks among equally qualified props (spec: "date_seed only tie-breaks among equally qualified props" — sanctioned visual-texture use). Candidates sort by prop id string for cross-run determinism, then `date_seed % len` picks.
- **Pause lives inside the pure function**: `day.asleep => None`. Every consumer (panel bias + styling) pauses automatically and the rule is restart-idempotent.
- Panel consumption (sheet wiring rules): the panel render recomputes resonance from the vm-carried context — preview DayContext overrides flow through for free. `vm.habitat.earned_props` is `Vec<EarnedHabitatPropView>` (src/tui/view_model.rs:84-89) which today has NO `source` field — this task adds one so the panel can build the `&[EarnedHabitatProp]` argument.

**Files:**
- Modify: `src/game/habitat.rs` (pub re-export of `HabitatPropId`)
- Modify: `src/tui/day.rs` (`FEAST_DAY_RATIO`, `resonant_prop_for_day`, tests)
- Modify: `src/tui/view_model.rs` (`EarnedHabitatPropView.source` + fixture)
- Modify: `src/commands/watch.rs` (`build_habitat_view` carries source; vm test)
- Modify: `src/tui/component/habitat_props.rs` (test helper gains source)
- Modify: `src/dev_preview/habitat_props.rs` (`earned_view` gains source)
- Modify: `src/tui/panels/pet.rs` (resonance constants/helpers, wander bias, gentle styling, tests)

- [ ] **Step 1: Write the failing pure-function tests**

Read `src/tui/day.rs` tests mod first (opens at day.rs:535, closes at day.rs:977 post-T1 — both will have shifted after Tasks 1-9). Append inside the existing `mod tests`, at the end before the closing brace. The tests-mod needs one extra import next to the existing ones (day.rs:537-540):

```rust
    use crate::storage::state::HabitatPropId;
```

Then append:

```rust
    // ── resonant_prop_for_day ────────────────────────────────────────────

    fn earned_prop(
        id: &str,
        source: HabitatPropSource,
        earned_at: time::OffsetDateTime,
    ) -> EarnedHabitatProp {
        EarnedHabitatProp {
            id: HabitatPropId::new(id),
            earned_at,
            source,
        }
    }

    fn resonance_day(date_seed: u64, yesterday_ratio: f32) -> DayContext {
        DayContext {
            yesterday: Some(DaySummary {
                ratio: yesterday_ratio,
                dominant_shape: None,
            }),
            date_seed,
            mature: true,
            asleep: false,
            local_day_started_utc: datetime!(2026-06-08 00:00 UTC),
            ..DayContext::default()
        }
    }

    #[test]
    fn resonance_requires_an_earned_prop() {
        let day = resonance_day(0, 1.8);
        assert_eq!(resonant_prop_for_day(&day, &[]), None, "earned-only");
        // A lifetime-ladder trophy is a milestone, not a day character.
        let ladder = earned_prop(
            "token_pebble_25k",
            HabitatPropSource::LifetimeTokens {
                threshold: 25_000.0,
            },
            datetime!(2026-06-07 12:00 UTC),
        );
        assert_eq!(resonant_prop_for_day(&day, &[ladder]), None);
    }

    #[test]
    fn feast_yesterday_resonates_the_heavy_session_planter_only_when_mature() {
        let planter = earned_prop(
            "heavy_session_planter",
            HabitatPropSource::HeavySession,
            datetime!(2026-05-01 12:00 UTC), // earned long ago: the ratio requalifies it
        );
        let day = resonance_day(0, 1.8);
        assert_eq!(
            resonant_prop_for_day(&day, &[planter.clone()]),
            Some(HabitatPropId::new("heavy_session_planter"))
        );
        let immature = DayContext {
            mature: false,
            ..day
        };
        assert_eq!(
            resonant_prop_for_day(&immature, &[planter]),
            None,
            "ratio-qualified resonance is maturity-gated"
        );
    }

    #[test]
    fn codex_heavy_yesterday_and_fresh_recovery_resonate_without_maturity() {
        // Yesterday ran codex-dominant; wilt recovery happened this morning.
        let lamp = earned_prop(
            "codex_signal_lamp",
            HabitatPropSource::ProviderFirstUse {
                provider_surface: "codex".to_string(),
            },
            datetime!(2026-05-20 15:00 UTC), // earned long ago: the share requalifies it
        );
        let sprout = earned_prop(
            "wilt_recovery_sprout",
            HabitatPropSource::WiltRecovery,
            datetime!(2026-06-08 08:30 UTC),
        );
        let day = DayContext {
            mature: false, // neither channel is baseline-ratio-scaled
            yesterday_codex_share: 0.8,
            ..resonance_day(0, 0.2)
        };
        assert_eq!(
            resonant_prop_for_day(&day, &[lamp]),
            Some(HabitatPropId::new("codex_signal_lamp")),
            "codex-dominant yesterday requalifies the lamp — recurring, not once-ever"
        );
        assert_eq!(
            resonant_prop_for_day(&day, &[sprout]),
            Some(HabitatPropId::new("wilt_recovery_sprout"))
        );
    }

    #[test]
    fn stale_event_unlocks_and_ordinary_yesterdays_resonate_nothing() {
        let lamp = earned_prop(
            "codex_signal_lamp",
            HabitatPropSource::ProviderFirstUse {
                provider_surface: "codex".to_string(),
            },
            datetime!(2026-05-20 15:00 UTC), // and yesterday_codex_share stays 0.0
        );
        let planter = earned_prop(
            "heavy_session_planter",
            HabitatPropSource::HeavySession,
            datetime!(2026-05-01 12:00 UTC),
        );
        let day = resonance_day(0, 0.3); // ordinary yesterday: no feast
        assert_eq!(
            resonant_prop_for_day(&day, &[lamp, planter]),
            None,
            "no qualifying signal -> no companion"
        );
    }

    #[test]
    fn date_seed_tie_breaks_equally_qualified_props_deterministically() {
        let lamp = earned_prop(
            "codex_signal_lamp",
            HabitatPropSource::ProviderFirstUse {
                provider_surface: "codex".to_string(),
            },
            datetime!(2026-05-20 15:00 UTC), // qualifies via codex share below
        );
        let planter = earned_prop(
            "heavy_session_planter",
            HabitatPropSource::HeavySession,
            datetime!(2026-05-01 12:00 UTC), // feast yesterday: qualifies
        );
        let earned = [lamp, planter];
        let seed_zero = DayContext {
            yesterday_codex_share: 0.8,
            ..resonance_day(0, 1.8)
        };
        let seed_one = DayContext {
            yesterday_codex_share: 0.8,
            ..resonance_day(1, 1.8)
        };
        // Candidates sort by id: ["codex_signal_lamp", "heavy_session_planter"].
        assert_eq!(
            resonant_prop_for_day(&seed_zero, &earned),
            Some(HabitatPropId::new("codex_signal_lamp"))
        );
        assert_eq!(
            resonant_prop_for_day(&seed_one, &earned),
            Some(HabitatPropId::new("heavy_session_planter"))
        );
        assert_eq!(
            resonant_prop_for_day(&seed_zero, &earned),
            resonant_prop_for_day(&seed_zero, &earned),
            "same day, same companion"
        );
    }

    #[test]
    fn resonance_pauses_entirely_while_asleep() {
        let planter = earned_prop(
            "heavy_session_planter",
            HabitatPropSource::HeavySession,
            datetime!(2026-05-01 12:00 UTC),
        );
        let day = DayContext {
            asleep: true,
            ..resonance_day(0, 1.8)
        };
        assert_eq!(
            resonant_prop_for_day(&day, &[planter]),
            None,
            "no glowing shrine over a sleeping pet"
        );
    }
```

- [ ] **Step 2: Run the tests — confirm red**

```bash
cargo test --lib resonan 2>&1 | tail -20
```

Expected: compile error — `error[E0425]: cannot find function `resonant_prop_for_day`` (and `EarnedHabitatProp`/`HabitatPropSource` unresolved in day.rs's tests until the module import lands in Step 3).

- [ ] **Step 3: Implement the re-export and the pure function**

3a. `src/game/habitat.rs` — Read the import block first (habitat.rs:3-9 post-T1: `use crate::{ game::metabolism::Mood, storage::{ state::{EarnedHabitatProp, HabitatPropId, HabitatPropSource, PetState}, usage_store::UsageLedgerRow, }, };`). Remove `HabitatPropId` from the inner braces and add a re-export directly below the use block:

```rust
use crate::{
    game::metabolism::Mood,
    storage::{
        state::{EarnedHabitatProp, HabitatPropSource, PetState},
        usage_store::UsageLedgerRow,
    },
};

/// Re-exported so resonance consumers can name the id type through the
/// habitat module that owns the catalog (interface sheet path).
pub use crate::storage::state::HabitatPropId;
```

3b. `src/tui/day.rs` — extend the module import (day.rs:11 post-T1 reads `use crate::storage::state::PetState;`):

```rust
use crate::storage::state::{EarnedHabitatProp, HabitatPropSource, PetState};
```

3c. Verify `FEAST_DAY_RATIO` already exists in the day.rs constants block —
Task 5 added it (shared with morning-after flavor). Do NOT redeclare; if it
is missing, Task 5 was executed incompletely — fix there, not here.

3d. Add the function after `classify_day_shape` (ends day.rs:412 post-T1):

```rust
/// The earned prop whose unlock provenance matches yesterday/climate
/// (heavy-session planter after a feast day, codex lamp after codex-heavy,
/// recovery sprout after wilt recovery); date_seed tie-breaks among equally
/// qualified; None when no signal qualifies. Ratio-qualified matches require
/// day.mature.
///
/// Matching by provenance kind:
/// - HeavySession: qualifies when yesterday was a feast
///   (yesterday.ratio >= FEAST_DAY_RATIO) — baseline-ratio, maturity-gated.
/// - ProviderFirstUse (codex lamp): qualifies when yesterday's applied usage
///   was codex-dominant (day.yesterday_codex_share >= CODEX_HEAVY_SHARE) —
///   source-mix-qualified, recurring, no maturity gate.
/// - WiltRecovery: one-shot event — qualifies while the unlock is fresh
///   (earned within yesterday-or-today, judged against the carried
///   local_day_started_utc; the fixed 24h step is at most an hour off
///   across DST). Event-qualified, so no maturity gate.
/// - LifetimeTokens: a ladder milestone, never a day character.
/// Paused entirely while asleep (spec: no glowing shrine over a sleeping pet).
pub fn resonant_prop_for_day(
    day: &DayContext,
    earned: &[crate::storage::state::EarnedHabitatProp],
) -> Option<crate::game::habitat::HabitatPropId> {
    if day.asleep {
        return None;
    }
    let yesterday_started = day.local_day_started_utc - Duration::days(1);
    let feast_yesterday = day.mature
        && day
            .yesterday
            .is_some_and(|summary| summary.ratio >= FEAST_DAY_RATIO);
    let codex_heavy_yesterday = day.yesterday_codex_share >= CODEX_HEAVY_SHARE;
    let mut qualified: Vec<&EarnedHabitatProp> = earned
        .iter()
        .filter(|prop| match &prop.source {
            HabitatPropSource::HeavySession => feast_yesterday,
            HabitatPropSource::ProviderFirstUse { .. } => codex_heavy_yesterday,
            HabitatPropSource::WiltRecovery => prop.earned_at >= yesterday_started,
            HabitatPropSource::LifetimeTokens { .. } => false,
        })
        .collect();
    if qualified.is_empty() {
        return None;
    }
    qualified.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    let index = (day.date_seed % qualified.len() as u64) as usize;
    Some(qualified[index].id.clone())
}
```

- [ ] **Step 4: Run the tests — confirm green**

```bash
cargo test --lib resonan 2>&1 | tail -5
```

Expected: `test result: ok. 6 passed` (filter matches the six new tests).

- [ ] **Step 5: Write the failing vm-provenance test**

The panel needs unlock provenance on the view. Drive it through the real vm build. Append to the existing `mod tests` in `src/commands/watch.rs` (vm-level tests live there, e.g. `status_today_and_watch_today_agree_across_a_midnight_boundary` at watch.rs:1072 post-T1 — Read the mod for current imports; `tempdir`, `UsageStore`, `PetState`, `LocalDayMapper`, `OffsetDateTime` are already in scope):

```rust
    #[test]
    fn habitat_view_carries_unlock_provenance_for_resonance() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("usage.sqlite");
        drop(UsageStore::open(&db_path).unwrap());
        let mut state = PetState::new_for_test("seed", "buddy");
        state.habitat.earned_props = vec![crate::storage::state::EarnedHabitatProp {
            id: crate::storage::state::HabitatPropId::new(
                crate::game::habitat::HEAVY_SESSION_PLANTER,
            ),
            earned_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            source: crate::storage::state::HabitatPropSource::HeavySession,
        }];
        let now = OffsetDateTime::from_unix_timestamp(1_700_000_600).unwrap();
        let vm = build_watch_view_model_at(
            &state,
            &db_path,
            now,
            LocalDayMapper::Fixed(time::UtcOffset::UTC),
        )
        .unwrap();
        assert_eq!(
            vm.habitat.earned_props[0].source,
            crate::storage::state::HabitatPropSource::HeavySession,
            "the habitat view must carry provenance so the panel can match resonance"
        );
    }
```

- [ ] **Step 6: Run — confirm red**

```bash
cargo test --lib habitat_view_carries_unlock_provenance 2>&1 | tail -10
```

Expected: compile error `error[E0609]: no field `source` on type `EarnedHabitatPropView``.

- [ ] **Step 7: Add `source` to the view and all construction sites**

7a. `src/tui/view_model.rs` — Read the struct (view_model.rs:84-89) and the file's imports first. Extend the storage-state import to include `HabitatPropSource` (alongside the existing `HabitatPropId` import wherever it lives), then:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct EarnedHabitatPropView {
    pub id: HabitatPropId,
    pub earned_at: time::OffsetDateTime,
    pub kind: HabitatPropKind,
    pub display_priority: i16,
    /// Unlock provenance, carried so the panel can match prop resonance.
    pub source: HabitatPropSource,
}
```

7b. `src/tui/view_model.rs` fixture `fixture_with_habitat_props` (view_model.rs:279-296) — the two literals gain their true sources:

```rust
            EarnedHabitatPropView {
                id: crate::storage::state::HabitatPropId::new("codex_signal_lamp"),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: crate::game::habitat::HabitatPropKind::Trophy,
                display_priority: 70,
                source: HabitatPropSource::ProviderFirstUse {
                    provider_surface: "codex".to_string(),
                },
            },
            EarnedHabitatPropView {
                id: crate::storage::state::HabitatPropId::new("token_pebble_25k"),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: crate::game::habitat::HabitatPropKind::Accent,
                display_priority: 10,
                source: HabitatPropSource::LifetimeTokens {
                    threshold: 25_000.0,
                },
            },
```

7c. `src/commands/watch.rs` `build_habitat_view` (watch.rs:292-309) — the mapped literal gains:

```rust
            Some(EarnedHabitatPropView {
                id: earned.id.clone(),
                earned_at: earned.earned_at,
                kind: spec.kind,
                display_priority: spec.display_priority,
                source: earned.source.clone(),
            })
```

7d. `src/tui/component/habitat_props.rs` test helper `earned` (habitat_props.rs:977-984) — provenance is irrelevant to layering tests; use the neutral ladder source (add `use crate::storage::state::HabitatPropSource;` to the tests-mod imports at habitat_props.rs:950):

```rust
    fn earned(id: &str, kind: HabitatPropKind, priority: i16, minute: u8) -> EarnedHabitatPropView {
        EarnedHabitatPropView {
            id: HabitatPropId::new(id),
            earned_at: datetime!(2026-05-11 12:00 UTC) + time::Duration::minutes(i64::from(minute)),
            kind,
            display_priority: priority,
            source: HabitatPropSource::LifetimeTokens { threshold: 0.0 },
        }
    }
```

7e. `src/dev_preview/habitat_props.rs` `earned_view` (dev_preview/habitat_props.rs:165-172) — derive the true provenance from the catalog spec (add `use crate::storage::state::HabitatPropSource;` to the imports near line 17):

```rust
fn earned_view(prop: &HabitatPropSpec, earned_at: OffsetDateTime) -> EarnedHabitatPropView {
    let source = match prop.lifetime_threshold {
        Some(threshold) => HabitatPropSource::LifetimeTokens { threshold },
        None => match prop.id {
            crate::game::habitat::CODEX_SIGNAL_LAMP => HabitatPropSource::ProviderFirstUse {
                provider_surface: "codex".to_string(),
            },
            crate::game::habitat::WILT_RECOVERY_SPROUT => HabitatPropSource::WiltRecovery,
            _ => HabitatPropSource::HeavySession,
        },
    };
    EarnedHabitatPropView {
        id: HabitatPropId::new(prop.id),
        earned_at,
        kind: prop.kind,
        display_priority: prop.display_priority,
        source,
    }
}
```

- [ ] **Step 8: Run — confirm green**

```bash
cargo test --lib habitat_view_carries_unlock_provenance 2>&1 | tail -5
cargo test --lib habitat 2>&1 | tail -5
```

Expected: both `test result: ok` (the second confirms the layering tests in habitat_props.rs still pass).

- [ ] **Step 9: Write the failing panel-helper tests**

Append to `mod tests` in `src/tui/panels/pet.rs` (opens at pet.rs:1271 post-T1; `use super::*` is in effect, so the new helpers and `PropReactionKind` resolve once implemented):

```rust
    // ── prop resonance consumption ───────────────────────────────────────

    #[test]
    fn resonance_adds_gentle_glow_when_prop_has_no_live_reaction() {
        let id = crate::storage::state::HabitatPropId::new(
            crate::game::habitat::HEAVY_SESSION_PLANTER,
        );
        let styled = apply_resonance_reaction(PetLifeProfile::default(), Some(&id));
        assert_eq!(styled.prop_reactions.len(), 1);
        let reaction = &styled.prop_reactions[0];
        assert_eq!(reaction.prop_id, id);
        assert_eq!(reaction.kind, PropReactionKind::Glow);
        assert!(
            reaction.intensity > 0.0 && reaction.intensity <= 1.0,
            "gentle and inside the existing 0..=1 cap"
        );
    }

    #[test]
    fn resonance_never_overrides_a_live_reaction_for_the_same_prop() {
        let id = crate::storage::state::HabitatPropId::new(
            crate::game::habitat::HEAVY_SESSION_PLANTER,
        );
        let profile = PetLifeProfile {
            prop_reactions: vec![PropReaction {
                prop_id: id.clone(),
                intensity: 0.72,
                kind: PropReactionKind::Bloom,
            }],
            ..PetLifeProfile::default()
        };
        let styled = apply_resonance_reaction(profile, Some(&id));
        assert_eq!(styled.prop_reactions.len(), 1, "no duplicate reaction");
        assert_eq!(styled.prop_reactions[0].intensity, 0.72, "live channel wins");
        assert_eq!(styled.prop_reactions[0].kind, PropReactionKind::Bloom);
    }

    #[test]
    fn resonance_wander_bias_points_toward_the_prop_zone() {
        // Catalog zones: planter FloorRight (habitat.rs:183), sprout FloorLeft (:192).
        let planter = crate::storage::state::HabitatPropId::new(
            crate::game::habitat::HEAVY_SESSION_PLANTER,
        );
        let sprout = crate::storage::state::HabitatPropId::new(
            crate::game::habitat::WILT_RECOVERY_SPROUT,
        );
        assert!(resonance_wander_bias(Some(&planter)) > 0, "right-zone prop pulls right");
        assert!(resonance_wander_bias(Some(&sprout)) < 0, "left-zone prop pulls left");
        assert_eq!(resonance_wander_bias(None), 0, "no companion, no bias");
    }
```

- [ ] **Step 10: Run — confirm red**

```bash
cargo test --lib resonance_ 2>&1 | tail -10
```

Expected: compile error `error[E0425]: cannot find function `apply_resonance_reaction``.

- [ ] **Step 11: Implement the panel helpers and wire the render path**

11a. `src/tui/panels/pet.rs` imports (pet.rs:11 and pet.rs:21 — Read first):

```rust
use crate::game::habitat::{catalog_prop, HabitatPetLayer, HabitatPropId, HabitatPropZone};
```
```rust
use crate::tui::life::{
    build_prop_reactions, PetLifeProfile, PropReaction, PropReactionKind, WorkWeather,
};
```

(`HabitatPropId` here exercises the Task 10 re-export — the sheet path.)

11b. Constants + helpers, placed directly after `activity_glyph_budget` (pet.rs:128-135):

```rust
/// Gentle resonance glow intensity — well inside the existing 0..=1 reaction cap.
const RESONANCE_REACTION_INTENSITY: f32 = 0.25;
/// Cells of horizontal wander bias toward the resonant prop's habitat side.
/// The render-time clamp (pet art positioning) keeps the pet in-bounds.
const RESONANCE_WANDER_BIAS_CELLS: i16 = 3;

/// Adds a gentle Glow for the day's resonant prop. A live reaction for the
/// same prop always wins (live-activity channels outrank day flavor).
fn apply_resonance_reaction(
    mut profile: PetLifeProfile,
    resonant: Option<&HabitatPropId>,
) -> PetLifeProfile {
    let Some(id) = resonant else {
        return profile;
    };
    if profile.prop_reactions.iter().any(|r| r.prop_id == *id) {
        return profile;
    }
    profile.prop_reactions.push(PropReaction {
        prop_id: id.clone(),
        intensity: RESONANCE_REACTION_INTENSITY,
        kind: PropReactionKind::Glow,
    });
    profile
}

/// Signed wander bias toward the resonant prop's catalog zone side.
fn resonance_wander_bias(resonant: Option<&HabitatPropId>) -> i16 {
    let Some(spec) = resonant.and_then(catalog_prop) else {
        return 0;
    };
    let side: i16 = match spec.zone {
        HabitatPropZone::FloorLeft | HabitatPropZone::WallLeft | HabitatPropZone::AirLeft => -1,
        HabitatPropZone::FloorRight | HabitatPropZone::WallRight | HabitatPropZone::AirRight => 1,
        HabitatPropZone::FloorMid | HabitatPropZone::AirMid | HabitatPropZone::Ceiling => 0,
    };
    side * RESONANCE_WANDER_BIAS_CELLS
}
```

11c. In `PetPanel::render` — Read the region first (post-T1 anchor pet.rs:584-606: `let day = &vm.day_context;` followed by the `(wander_x, facing)` match). Insert after `let day = &vm.day_context;`:

```rust
        // Prop resonance: recomputed from the vm-carried context so preview
        // overrides and production agree. None while asleep (full pause).
        let resonant_prop = {
            let earned: Vec<crate::storage::state::EarnedHabitatProp> = vm
                .habitat
                .earned_props
                .iter()
                .map(|prop| crate::storage::state::EarnedHabitatProp {
                    id: prop.id.clone(),
                    earned_at: prop.earned_at,
                    source: prop.source.clone(),
                })
                .collect();
            crate::tui::day::resonant_prop_for_day(day, &earned)
        };
```

11d. Bias ONLY the awake default arm of the wander match (the asleep-hold and wake-ease arms keep their exact T1 semantics; resonance is None while asleep anyway):

```rust
            _ => (
                compute_wander_position_x(area.width, species, now)
                    + resonance_wander_bias(resonant_prop.as_ref()),
                compute_facing(area.width, species, now),
            ),
```

11e. Gentle styling after the existing `build_prop_reactions` call (pet.rs:667):

```rust
        let life_profile = build_prop_reactions(vm.life_profile.clone(), &earned_prop_ids, compact);
        let life_profile = apply_resonance_reaction(life_profile, resonant_prop.as_ref());
```

(`apply_prop_reaction_style` at pet.rs:299-303 already passes Flat through untouched and caps the fg lift by `intensity * 35` — the resonance Glow rides entirely inside existing caps.)

- [ ] **Step 12: Run — confirm green, then the full suite**

```bash
cargo test --lib resonance_ 2>&1 | tail -5
cargo test 2>&1 | tail -5
```

Expected: `test result: ok` on both; no other suite regressions (the view field is additive; all construction sites were updated in Step 7).

- [ ] **Step 13: fmt, clippy, commit**

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
git add -u
git commit -m "feat(tui): keep company with yesterday's resonant prop

resonant_prop_for_day matches earned-prop provenance against the day:
feast yesterday requalifies the heavy-session planter (maturity-gated),
fresh ProviderFirstUse/WiltRecovery unlocks resonate on their own,
date_seed tie-breaks, asleep pauses everything. The panel consumes it
as a bounded wander bias plus a gentle glow inside existing caps.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 11: Preview Lab — T2+T3 daycontext fixtures, manifest contracts, snapshot

**Spec section:** Testing and proof, "Preview Lab fixtures" (spec:496-500); Boundary behavior (midnight tidy fade, spec:300-309); climate fixture requirement (spec:95-96).

Seven new deterministic watch fixtures, registered exactly like the four shipped T1 daycontext fixtures (override seam src/dev_preview/watch.rs:181-227, fixture vec :315-375, builders :591-634, metadata arms src/dev_preview/scenarios.rs:542+, ordered pins watch.rs:667-698 / scenarios.rs:784-823 / tests/dev_preview.rs). All five registration surfaces must move together. `ctx.fixed_now` is unix 1_760_000_000 (2025-10-09 08:53:20 UTC, scenarios.rs:41) — second 20 of the 30s speech cycle, so bubbles are invisible at `fixed_now`; speech-bearing fixtures shift `now` by +10s to land at cycle second 0.

New ids, in registration order (all 120x32, all rendered from `liveliness_pet_state` — S6 Crystal with the codex lamp, planter, pebble, and shell earned, watch.rs:443-476):

| id | shows |
|---|---|
| `watch-daycontext-dream-night` | asleep + yesterday `dominant_shape` detail → dream window |
| `watch-daycontext-heavy-day-evening` | tiredness 0.85 + dense (capped) motes at Dusk — **snapshot** |
| `watch-daycontext-light-day-morning` | Dawn morning-after, idle-yesterday flavor (`Some`, ratio≈0) |
| `watch-daycontext-weekend-midday` | full weekend softening (share 0.05 ≤ WEEKEND_QUIET_SHARE) |
| `watch-daycontext-climate-cache-week` | `climate: Some(CacheMist)` ambient tint bias |
| `watch-daycontext-prop-resonance-planter` | feast yesterday → planter glow + wander bias |
| `watch-daycontext-midnight-mid-session` | awake at night, 10 min after rollover → mote tidy fade |

**Files:**
- Modify: `src/dev_preview/watch.rs` (seam, fixtures, builders, ordered pin test)
- Modify: `src/dev_preview/scenarios.rs` (metadata arms + manifest id pin)
- Test: `tests/dev_preview.rs` (id constant, file list, id list, new snapshot)
- Created by test run: `tests/snapshots/dev_preview__watch_daycontext_heavy_day_evening_frame.snap`

- [ ] **Step 1: Update all ordered pins to the new totals — these are the failing tests**

1a. `src/dev_preview/watch.rs` test `watch_frames_include_wide_tall_wide_and_compact` (watch.rs:667-698): change `assert_eq!(frames.len(), 14);` to `assert_eq!(frames.len(), 21);` and append after the `frames[13]` asserts:

```rust
        assert_eq!(frames[14].id, "watch-daycontext-dream-night");
        assert_eq!((frames[14].width, frames[14].height), (120, 32));
        assert_eq!(frames[15].id, "watch-daycontext-heavy-day-evening");
        assert_eq!((frames[15].width, frames[15].height), (120, 32));
        assert_eq!(frames[16].id, "watch-daycontext-light-day-morning");
        assert_eq!((frames[16].width, frames[16].height), (120, 32));
        assert_eq!(frames[17].id, "watch-daycontext-weekend-midday");
        assert_eq!((frames[17].width, frames[17].height), (120, 32));
        assert_eq!(frames[18].id, "watch-daycontext-climate-cache-week");
        assert_eq!((frames[18].width, frames[18].height), (120, 32));
        assert_eq!(frames[19].id, "watch-daycontext-prop-resonance-planter");
        assert_eq!((frames[19].width, frames[19].height), (120, 32));
        assert_eq!(frames[20].id, "watch-daycontext-midnight-mid-session");
        assert_eq!((frames[20].width, frames[20].height), (120, 32));
```

1b. `src/dev_preview/scenarios.rs` test `all_selection_writes_watch_and_pet_scenarios` (scenarios.rs:784-823): in the pinned `ids` vec, insert after `"watch-daycontext-hatch-at-night",`:

```rust
                "watch-daycontext-dream-night",
                "watch-daycontext-heavy-day-evening",
                "watch-daycontext-light-day-morning",
                "watch-daycontext-weekend-midday",
                "watch-daycontext-climate-cache-week",
                "watch-daycontext-prop-resonance-planter",
                "watch-daycontext-midnight-mid-session",
```

(The manifest grows from 20 to 27 ids.)

1c. `tests/dev_preview.rs` — three edits:

Replace the id constant (tests/dev_preview.rs:19-24; the array length is in the type):

```rust
const DAY_CONTEXT_WATCH_IDS: [&str; 11] = [
    "watch-daycontext-night-asleep",
    "watch-daycontext-dawn-crossing",
    "watch-daycontext-night-wake-catchup",
    "watch-daycontext-hatch-at-night",
    "watch-daycontext-dream-night",
    "watch-daycontext-heavy-day-evening",
    "watch-daycontext-light-day-morning",
    "watch-daycontext-weekend-midday",
    "watch-daycontext-climate-cache-week",
    "watch-daycontext-prop-resonance-planter",
    "watch-daycontext-midnight-mid-session",
];
```

In `dev_preview_all_writes_watch_and_pet_artifacts` (tests/dev_preview.rs:564-622): in the frames file list, insert after `"frames/watch-daycontext-hatch-at-night.txt",`:

```rust
        "frames/watch-daycontext-dream-night.txt",
        "frames/watch-daycontext-heavy-day-evening.txt",
        "frames/watch-daycontext-light-day-morning.txt",
        "frames/watch-daycontext-weekend-midday.txt",
        "frames/watch-daycontext-climate-cache-week.txt",
        "frames/watch-daycontext-prop-resonance-planter.txt",
        "frames/watch-daycontext-midnight-mid-session.txt",
```

and in the pinned ids vec, insert after `"watch-daycontext-hatch-at-night".to_string(),`:

```rust
            "watch-daycontext-dream-night".to_string(),
            "watch-daycontext-heavy-day-evening".to_string(),
            "watch-daycontext-light-day-morning".to_string(),
            "watch-daycontext-weekend-midday".to_string(),
            "watch-daycontext-climate-cache-week".to_string(),
            "watch-daycontext-prop-resonance-planter".to_string(),
            "watch-daycontext-midnight-mid-session".to_string(),
```

- [ ] **Step 2: Run — confirm red**

```bash
cargo test watch_frames_include_wide_tall_wide_and_compact 2>&1 | tail -8
```

Expected: FAILED — `assertion `left == right` failed: left: 14, right: 21`.

- [ ] **Step 3: Implement the fixtures**

3a. `src/dev_preview/watch.rs` import (line 13): add `DaySummary`:

```rust
use crate::tui::day::{DayContext, DayPhase, DaySummary, WakeResume};
```

3b. Extend the override seam in `render_watch_frame_from_state_with_life` (watch.rs:210-216 — Read first; Tasks 1-9 changed `current_pet_speech_for_scene` to the 4-arg `&DayContext` signature). The day override must also restamp speech (so dream/morning-after bubbles are the production selector's real output) and re-render the pet when tiredness is in play (so `blink_slowdown` from `vm.day_context.tiredness` reaches the frame):

```rust
    if let Some(day) = day_context {
        vm.day_context = day;
        vm.life_profile.calm_mode = day.asleep;
        // Re-stamp speech from the overridden context so fixture frames show
        // exactly what the production selector would emit for this scene.
        vm.current_speech = crate::pet::speech::current_pet_speech_for_scene(
            vm.pet_render.mood,
            &vm.life_profile,
            &day,
            now,
        );
    }
    if hold_eyes_closed || day_context.is_some_and(|day| day.tiredness > 0.0) {
        rerender_pet_for_view_model(&mut vm, now.unix_timestamp().max(0) as u64, hold_eyes_closed)?;
    }
```

(`DayContext` is `Copy`, so `day_context` stays usable after the `if let`.)

3c. Append seven entries to `day_context_frame_fixtures` (watch.rs:315-375), after the hatch-at-night entry. First add, right after `let fixed_now = ctx.fixed_now;`:

```rust
    // fixed_now sits at second 20 of the 30s speech cycle (bubbles invisible);
    // +10s lands at second 0 so dream/morning-after bubbles can render.
    let speech_visible_now = fixed_now + Duration::seconds(10);
```

then the entries:

```rust
        DayContextFrameFixture {
            id: "watch-daycontext-dream-night",
            title: "Watch DayContext Dream Night",
            width: 120,
            height: 32,
            now: speech_visible_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: calm_idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: dream_night_day_context(speech_visible_now),
            hold_eyes_closed: true,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-heavy-day-evening",
            title: "Watch DayContext Heavy Day Evening",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: cooling_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: heavy_day_evening_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-light-day-morning",
            title: "Watch DayContext Light Day Morning",
            width: 120,
            height: 32,
            now: speech_visible_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: light_day_morning_day_context(speech_visible_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-weekend-midday",
            title: "Watch DayContext Weekend Midday",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: weekend_midday_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-climate-cache-week",
            title: "Watch DayContext Climate Cache Week",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: climate_cache_week_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-prop-resonance-planter",
            title: "Watch DayContext Prop Resonance Planter",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: prop_resonance_planter_day_context(fixed_now),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-daycontext-midnight-mid-session",
            title: "Watch DayContext Midnight Mid Session",
            width: 120,
            height: 32,
            now: fixed_now,
            state: liveliness_pet_state,
            life: WatchLifeFixture {
                profile: warm_life_profile(false),
                color_capability: ColorCapability::Truecolor,
            },
            day_context: midnight_mid_session_day_context(fixed_now),
            hold_eyes_closed: false,
        },
```

3d. Builders, appended after `night_newborn_day_context` (watch.rs:626-634), following the `..DayContext::default()` struct-update convention:

```rust
fn dream_night_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Night,
        phase_started_at_utc: now - Duration::hours(3),
        phase_ends_at_utc: now + Duration::hours(5),
        asleep: true,
        sleep_onset_utc: Some(now - Duration::hours(1)),
        yesterday: Some(DaySummary {
            ratio: 1.6,
            dominant_shape: Some(WorkWeather::CacheMist),
        }),
        date_seed: 7,
        mature: true,
        local_day_started_utc: now - Duration::hours(2),
        local_day_rollover_utc: now + Duration::hours(22),
        ..DayContext::default()
    }
}

fn heavy_day_evening_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Dusk,
        phase_started_at_utc: now - Duration::hours(1),
        phase_ends_at_utc: now + Duration::hours(2),
        today_ratio: 1.7,
        tiredness: 0.85,
        mature: true,
        local_day_started_utc: now - Duration::hours(13),
        local_day_rollover_utc: now + Duration::hours(11),
        ..DayContext::default()
    }
}

fn light_day_morning_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Dawn,
        phase_started_at_utc: now - Duration::minutes(40),
        phase_ends_at_utc: now + Duration::minutes(80),
        today_ratio: 0.02,
        yesterday: Some(DaySummary {
            ratio: 0.04,
            dominant_shape: None,
        }),
        mature: true,
        local_day_started_utc: now - Duration::hours(7),
        local_day_rollover_utc: now + Duration::hours(17),
        ..DayContext::default()
    }
}

fn weekend_midday_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Day,
        phase_started_at_utc: now - Duration::hours(3),
        phase_ends_at_utc: now + Duration::hours(5),
        is_weekend: true,
        weekend_share: 0.05,
        today_ratio: 0.3,
        mature: true,
        local_day_started_utc: now - Duration::hours(12),
        local_day_rollover_utc: now + Duration::hours(12),
        ..DayContext::default()
    }
}

fn climate_cache_week_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Day,
        phase_started_at_utc: now - Duration::hours(3),
        phase_ends_at_utc: now + Duration::hours(5),
        climate: Some(WorkWeather::CacheMist),
        today_ratio: 0.5,
        mature: true,
        local_day_started_utc: now - Duration::hours(12),
        local_day_rollover_utc: now + Duration::hours(12),
        ..DayContext::default()
    }
}

fn prop_resonance_planter_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Day,
        phase_started_at_utc: now - Duration::hours(3),
        phase_ends_at_utc: now + Duration::hours(5),
        yesterday: Some(DaySummary {
            ratio: 1.9,
            dominant_shape: Some(WorkWeather::OutputSparks),
        }),
        today_ratio: 0.4,
        mature: true,
        local_day_started_utc: now - Duration::hours(12),
        local_day_rollover_utc: now + Duration::hours(12),
        ..DayContext::default()
    }
}

fn midnight_mid_session_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Night,
        phase_started_at_utc: now - Duration::hours(2),
        phase_ends_at_utc: now + Duration::hours(6),
        asleep: false,
        today_ratio: 0.03,
        tiredness: 0.45,
        yesterday: Some(DaySummary {
            ratio: 1.3,
            dominant_shape: Some(WorkWeather::Mixed),
        }),
        mature: true,
        local_day_started_utc: now - Duration::minutes(10),
        local_day_rollover_utc: now + Duration::hours(24) - Duration::minutes(10),
        ..DayContext::default()
    }
}
```

(Resonance check for the planter fixture: yesterday 1.9 ≥ FEAST_DAY_RATIO with `mature` → planter qualifies; the lamp was earned 12 days ago in `liveliness_pet_state` so its fresh-unlock window has long passed; planter is the sole companion — no tie-break ambiguity in the frame.)

3e. `src/dev_preview/scenarios.rs` — extend `day_context_inputs_for_frame` (scenarios.rs:542+, Read the whole function first). Widen the destructured match tuple by one trailing element so each fixture also publishes its T2/T3 day inputs:

```rust
    let (
        day_phase,
        phase_started_at_utc,
        phase_ends_at_utc,
        asleep,
        sleep_onset_utc,
        wake_resume,
        blend,
        life_profile,
        day_extras,
    ) = match id {
```

For each of the **five existing arms** (`night-asleep`, `dawn-crossing`, `night-wake-catchup`, `hatch-at-night`, and the `_` fallback) append `json!({})` as the new final tuple element, after the `life_profile` `json!({...})` — empty extras merge to nothing, so existing manifest output is unchanged.

Add the seven new arms before the `_` fallback:

```rust
        "watch-daycontext-dream-night" => {
            let dream_now = fixed_now + time::Duration::seconds(10);
            (
                "night",
                dream_now - time::Duration::hours(3),
                dream_now + time::Duration::hours(5),
                true,
                Some(dream_now - time::Duration::hours(1)),
                None,
                1.0,
                json!({
                    "activity_level": 0.0,
                    "burst_level": 0.0,
                    "source_accent": None::<&str>,
                    "weather": "clear",
                    "stage": "s6",
                    "species": "crystal",
                    "prop_reactions": json!([]),
                    "color_capability": "truecolor",
                    "calm_mode": true,
                    "freshness": "live"
                }),
                json!({
                    "mature": true,
                    "date_seed": 7,
                    "today_ratio": 0.0,
                    "tiredness": 0.0,
                    "yesterday": { "ratio": 1.6, "dominant_shape": "cache-mist" },
                    "climate": null,
                    "is_weekend": false,
                    "weekend_share": 0.0,
                    "local_day_started_utc":
                        format_rfc3339_lossy(dream_now - time::Duration::hours(2))
                }),
            )
        }
        "watch-daycontext-heavy-day-evening" => (
            "dusk",
            fixed_now - time::Duration::hours(1),
            fixed_now + time::Duration::hours(2),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.38,
                "burst_level": 0.0,
                "source_accent": Some("claude"),
                "weather": "cache-mist",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([{
                    "prop_id": "token_shell_100k",
                    "intensity": 0.28,
                    "kind": "glow"
                }]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "mature": true,
                "today_ratio": 1.7,
                "tiredness": 0.85,
                "yesterday": null,
                "climate": null,
                "is_weekend": false,
                "weekend_share": 0.0,
                "local_day_started_utc":
                    format_rfc3339_lossy(fixed_now - time::Duration::hours(13))
            }),
        ),
        "watch-daycontext-light-day-morning" => {
            let morning_now = fixed_now + time::Duration::seconds(10);
            (
                "dawn",
                morning_now - time::Duration::minutes(40),
                morning_now + time::Duration::minutes(80),
                false,
                None,
                None,
                1.0,
                json!({
                    "activity_level": 0.0,
                    "burst_level": 0.0,
                    "source_accent": None::<&str>,
                    "weather": "clear",
                    "stage": "s6",
                    "species": "crystal",
                    "prop_reactions": json!([]),
                    "color_capability": "truecolor",
                    "calm_mode": false,
                    "freshness": "live"
                }),
                json!({
                    "mature": true,
                    "today_ratio": 0.02,
                    "tiredness": 0.0,
                    "yesterday": { "ratio": 0.04, "dominant_shape": null },
                    "climate": null,
                    "is_weekend": false,
                    "weekend_share": 0.0,
                    "local_day_started_utc":
                        format_rfc3339_lossy(morning_now - time::Duration::hours(7))
                }),
            )
        }
        "watch-daycontext-weekend-midday" => (
            "day",
            fixed_now - time::Duration::hours(3),
            fixed_now + time::Duration::hours(5),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.0,
                "burst_level": 0.0,
                "source_accent": None::<&str>,
                "weather": "clear",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "mature": true,
                "today_ratio": 0.3,
                "tiredness": 0.0,
                "yesterday": null,
                "climate": null,
                "is_weekend": true,
                "weekend_share": 0.05,
                "local_day_started_utc":
                    format_rfc3339_lossy(fixed_now - time::Duration::hours(12))
            }),
        ),
        "watch-daycontext-climate-cache-week" => (
            "day",
            fixed_now - time::Duration::hours(3),
            fixed_now + time::Duration::hours(5),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.0,
                "burst_level": 0.0,
                "source_accent": None::<&str>,
                "weather": "clear",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "mature": true,
                "today_ratio": 0.5,
                "tiredness": 0.0,
                "yesterday": null,
                "climate": "cache-mist",
                "is_weekend": false,
                "weekend_share": 0.0,
                "local_day_started_utc":
                    format_rfc3339_lossy(fixed_now - time::Duration::hours(12))
            }),
        ),
        "watch-daycontext-prop-resonance-planter" => (
            "day",
            fixed_now - time::Duration::hours(3),
            fixed_now + time::Duration::hours(5),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.0,
                "burst_level": 0.0,
                "source_accent": None::<&str>,
                "weather": "clear",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "mature": true,
                "today_ratio": 0.4,
                "tiredness": 0.0,
                "yesterday": { "ratio": 1.9, "dominant_shape": "output-sparks" },
                "climate": null,
                "is_weekend": false,
                "weekend_share": 0.0,
                "local_day_started_utc":
                    format_rfc3339_lossy(fixed_now - time::Duration::hours(12)),
                // Derived value, included for reviewer convenience: the feast
                // yesterday requalifies the earned heavy-session planter.
                "resonant_prop": "heavy_session_planter"
            }),
        ),
        "watch-daycontext-midnight-mid-session" => (
            "night",
            fixed_now - time::Duration::hours(2),
            fixed_now + time::Duration::hours(6),
            false,
            None,
            None,
            1.0,
            json!({
                "activity_level": 0.68,
                "burst_level": 0.24,
                "source_accent": Some("balanced"),
                "weather": "mixed",
                "stage": "s6",
                "species": "crystal",
                "prop_reactions": json!([{
                    "prop_id": "codex_signal_lamp",
                    "intensity": 0.35,
                    "kind": "glow"
                }]),
                "color_capability": "truecolor",
                "calm_mode": false,
                "freshness": "live"
            }),
            json!({
                "mature": true,
                "today_ratio": 0.03,
                "tiredness": 0.45,
                "yesterday": { "ratio": 1.3, "dominant_shape": "mixed" },
                "climate": null,
                "is_weekend": false,
                "weekend_share": 0.0,
                "local_day_started_utc":
                    format_rfc3339_lossy(fixed_now - time::Duration::minutes(10))
            }),
        ),
```

Then merge the extras into the published `day_context` object — replace the `("day_context".to_string(), json!({...}))` BTreeMap entry with:

```rust
        (
            "day_context".to_string(),
            {
                let mut day_context_json = json!({
                    "day_phase": day_phase,
                    "phase_started_at_utc": format_rfc3339_lossy(phase_started_at_utc),
                    "phase_ends_at_utc": format_rfc3339_lossy(phase_ends_at_utc),
                    "asleep": asleep,
                    "sleep_onset_utc": sleep_onset_utc.map(format_rfc3339_lossy),
                    "wake_resume": wake_resume_json,
                    // `blend` is a computed value derived from `phase_started_at_utc`
                    // and `PHASE_BLEND_MINUTES`; it is included in the manifest for
                    // reviewer convenience.
                    "blend": blend
                });
                if let Value::Object(extra) = day_extras {
                    if let Value::Object(map) = &mut day_context_json {
                        map.extend(extra);
                    }
                }
                day_context_json
            },
        ),
```

- [ ] **Step 4: Run — confirm green**

```bash
cargo test watch_frames 2>&1 | tail -5
cargo test all_selection_writes_watch_and_pet_scenarios 2>&1 | tail -5
cargo test --test dev_preview 2>&1 | tail -5
```

Expected: all `test result: ok`. If `dev_preview_watch_daycontext_night_asleep_frame_snapshot` diffs, the only acceptable cause is the seam speech restamp — inspect the diff (`cargo insta pending-snapshots`, read the `.snap.new`), confirm the only change is the speech bubble region, then `cargo insta accept` and re-run. Any other diff is a regression: stop and debug.

- [ ] **Step 5: Add the failing whole-frame snapshot for heavy-day-evening**

Append to `tests/dev_preview.rs` after `dev_preview_watch_daycontext_night_asleep_frame_snapshot` (tests/dev_preview.rs:724-733):

```rust
#[test]
fn dev_preview_watch_daycontext_heavy_day_evening_frame_snapshot() {
    let run = PreviewRun::new();
    run.run_success("watch");

    let frame = std::fs::read_to_string(
        run.out
            .join("frames/watch-daycontext-heavy-day-evening.txt"),
    )
    .unwrap();

    insta::assert_snapshot!("watch_daycontext_heavy_day_evening_frame", frame);
}
```

- [ ] **Step 6: Run red, inspect, accept, run green**

```bash
cargo test --test dev_preview dev_preview_watch_daycontext_heavy_day_evening_frame_snapshot 2>&1 | tail -8
```

Expected: FAILED — insta stores the new frame as a pending snapshot. Read `tests/snapshots/dev_preview__watch_daycontext_heavy_day_evening_frame.snap.new` and verify: dusk-warm sky palette, floor motes present but visibly sub-half of the ambient field (MOTE_BUDGET_SHARE cap), no numbers or fill-direction anywhere, vitals bars untouched by tiredness. Then:

```bash
cargo insta accept
cargo test --test dev_preview 2>&1 | tail -5
```

Expected: `test result: ok`.

- [ ] **Step 7: Human review gate — open the contact sheet**

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```

Walk `review.md` and check, per new frame:

- `dream-night`: pet asleep (eyes closed, dim night palette); if the speech bubble is visible it must be a **dream line or zzz**, never a mood/munch line. If no dream bubble appears because date_seed 7 does not land a dream window on this hour, tune the fixture's `date_seed` (visual-texture knob only) until the dream renders, and mirror the new value in the scenarios.rs arm — then re-run Steps 4 and 6 (snapshots unaffected: dream-night has no snapshot).
- `heavy-day-evening`: motes read as ambient dust, sub-countable, no learnable "full room".
- `light-day-morning`: greeting (if visible) expresses the pet's own state — **never** the user's absence or yesterday's lowness (authoring guardrail).
- `weekend-midday`: softer palette/lazier feel than `climate-cache-week` at the same phase.
- `climate-cache-week`: ambient tint bias visible vs `weekend-midday`; nothing labels it.
- `prop-resonance-planter`: planter has a gentle glow; the pet sits biased toward the right floor (planter zone); no glow on the lamp or shells.
- `midnight-mid-session`: pet awake at night with live activity; a faint fading mote field from yesterday (tidy fade), no fresh dense motes.

Drew signs off on the contact sheet before commit.

- [ ] **Step 8: fmt, clippy, commit**

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
git status --short   # confirm only intended files (+ the new .snap) are staged-able
git add -u
git add tests/snapshots/dev_preview__watch_daycontext_heavy_day_evening_frame.snap
git commit -m "feat(preview): add T2+T3 daycontext fixtures to the watch contact sheet

Seven new deterministic frames (dream-night, heavy-day-evening,
light-day-morning, weekend-midday, climate-cache-week,
prop-resonance-planter, midnight-mid-session) with truthful DayContext
manifest inputs, a speech restamp + tiredness rerender in the preview
override seam, updated ordered pins (21 frames / 27 scenario ids), and
a whole-frame snapshot for heavy-day-evening.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 12: Final gate — full proof, spec-vs-diff walk, isolated live smoke

**Spec section:** Testing and proof (spec:466-505), Amendment (spec:311-355), "Considered and cut" (spec:425-441), Non-goals (spec:507-518).

No new features. This task proves the branch. Any fix it surfaces gets its own `fix(t2t3): ...` commit, after which the gate restarts from Step 1.

**Files:**
- None modified (verification only; fixes, if any, get their own commits)

- [ ] **Step 1: Full mechanical gate**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
npm test
```

Expected: fmt silent; clippy `Finished` with zero warnings; `cargo test` green across all ~14 suites with pristine output (no stray stderr noise — house rule); `npm test` green (runs cargo test + npm workspace smoke).

- [ ] **Step 2: Regenerate the preview bundle and do a final visual pass**

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```

Expected: 27 scenarios render; spot-check that nothing regressed in the T1 frames (night-asleep, dawn-crossing) next to the new T2/T3 frames.

- [ ] **Step 3: Spec-vs-diff walk — every bullet maps to a commit**

```bash
git log --oneline main..HEAD
git diff --stat main..HEAD
```

Walk this checklist; for each bullet name the commit(s) implementing it. A bullet with no commit is a gap — stop and raise it with Drew before merging:

| Spec bullet | Where it must show up |
|---|---|
| Amendment: usage discontinuity guard — per-surface sum, `max(guard_ratio x baseline x days_factor, 50M floor)`, per-provider `days_factor` from `provider_cursors.updated_at`, refused-alone semantics, first-contact refuse, ONE transaction (`refuse_poll_discontinuity`), "declined an implausible feast" feed line + `last_idle_narration_at` stamp, `usage_discontinuity` exempt from broken classification, config-overridable ratio | `src/game/runtime.rs` (`stage_usage_poll_deltas` new signature), `src/storage/usage_store.rs` (`latest_cursor_updated_at`, `refuse_poll_discontinuity`), both poll call sites |
| Amendment: local feed timestamps (`format_hhmm_local`, offset threaded from the mapper) | `src/pet/activity.rs`, `src/commands/watch.rs` (`timestamp_column` sites), `src/tui/app.rs` |
| T2: day-accumulation motes (soft saturation, `MOTE_BUDGET_SHARE` cap, date_seed jitter, tidy fade, maturity gate, Flat = zero) | `src/tui/panels/pet.rs` (`mote_glyphs_for`) |
| T2: tiredness (active-bucket counting in `FATIGUE_WINDOW_HOURS`, volume term, blink slowdown + breath lengthening, energy vital untouched, maturity gate) | `src/tui/day.rs` (`DayContext.tiredness`), `src/pet/render.rs` (`blink_slowdown`), `src/pet/animator.rs` (`BreathRhythm::Tired`) |
| T2: morning-after (`in_morning_after_window`, `Some(0.0)` vs `None` semantics, guardrailed lines, maturity gate) | `src/tui/day.rs`, `src/pet/speech.rs` |
| T2: dreams (only with `dominant_shape` detail; `DREAM_WINDOW_MINUTES` windows) | `src/pet/speech.rs` |
| T2: speech precedence stack (petting > dream/zzz > munch > needy mood > morning-after > default), both frontends | `src/pet/speech.rs` (`current_pet_speech_for_scene` 4-arg), `src/tui/app.rs`, `src/commands/watch.rs` |
| T3: sky character (`date_seed` family variants in `sky_palette_for_phase`) | `src/tui/panels/pet.rs` |
| T3: prop resonance (provenance matching, tie-break, asleep pause, wander bias + gentle glow) | Task 10 commit |
| T3: weekend texture (`weekend_softening` mapping, live channels win) | `src/tui/day.rs`, `src/tui/panels/pet.rs` |
| T3: climate rendering (`None`/`Clear` render nothing) | `src/tui/panels/pet.rs` (`sky_color_for_phase` climate param) |
| T3: seasons (subtle hue drift, never named) | `src/tui/panels/pet.rs` (`sky_color_for_phase` season param) |
| Preview fixtures + manifest inputs + snapshots | Task 11 commit |

- [ ] **Step 4: Explicitly check the cut items stayed cut**

```bash
# Within-stage body growth was cut: no generation/template changes allowed.
git diff --stat main..HEAD -- src/pet/generation.rs src/pet/art.rs
# Date-seeded speech vocabulary was cut: date_seed may pick dream MOMENTS,
# never phrase content. Read every hit and verify none indexes a phrase pool.
grep -n "date_seed" src/pet/speech.rs
# "Backfill cannot wake the pet" was cut and replaced by the bounded
# catch-up wake; no such predicate may exist.
grep -rni "cannot wake" src/
```

Expected: first command prints nothing (no diff); second command's hits (if any) are window/moment math only; third prints nothing. Also confirm the Non-goals grep-level: no streak/quota/ETA strings entered the UI (`grep -rn "streak\|quota\|ETA" src/ --include=*.rs` → only pre-existing hits, expected none).

- [ ] **Step 5: Live smoke against an ISOLATED config dir — never the real pet**

Rationale (spec:317-355): on 2026-06-10, ccusage 20.x silently became an all-agents aggregator and its first successful poll fed the real pet a **212M-effective-token bolus** of non-claude history. The discontinuity guard exists because of that incident (threshold ≈ max(5 x 19.77M x 1, 50M) ≈ 99M ≪ 212M → fires). Proving the guard by pointing this branch at `~/.config/glorp` would gamble Drew's actual pet on the very failure mode the guard defends against. The smoke ALWAYS pins `GLORP_CONFIG_DIR` to a throwaway dir.

```bash
SMOKE_DIR=$(mktemp -d)
GLORP_CONFIG_DIR="$SMOKE_DIR" cargo run --release -- init --yes --seed smoke --name smokey
GLORP_CONFIG_DIR="$SMOKE_DIR" cargo run --release -- status
```

Expected: init calibrates from real helper history without feeding it (calibration never grants XP); status renders a fresh pet (stage s0/s1, xp well under 1.0) against the real ccusage/ccusage-codex helpers, feed timestamps in local clock time, no panic, pristine output.

```bash
GLORP_CONFIG_DIR="$SMOKE_DIR" cargo run --release -- watch
# watch for ~60 seconds across at least 3 polls, then quit with q
```

Expected: live TUI runs against real helpers; day phase/sky match the actual local hour; any real usage lands as a normal feed event; no burst effects from stale data.

```bash
sqlite3 "$SMOKE_DIR/usage.sqlite" \
  "SELECT provider_surface, code, substr(message,1,80) FROM provider_diagnostics ORDER BY recorded_at DESC LIMIT 5;"
python3 -c "import json; d = json.load(open('$SMOKE_DIR/state.json')); print(d['stage'], d['xp'])"
```

Expected: zero or more diagnostics — if a `usage_discontinuity` row exists, the matching feed line is "smokey declined an implausible feast" and stage/xp did NOT jump (the guard refused instead of feeding); stage stays early and xp stays small for a minutes-old pet. Confirm `~/.config/glorp` was untouched:

```bash
ls -l ~/.config/glorp/state.json   # mtime must predate the smoke
```

Then clean up: `rm -rf "$SMOKE_DIR"`.

- [ ] **Step 6: fmt, clippy, close out**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
git status --short
```

Expected: all three silent — the gate itself changes nothing. If any step above surfaced a fix, it was committed as `fix(t2t3): <root cause>` (with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`) and the gate re-ran from Step 1; the branch ends green, clean, and walked. Hand off to Drew for the merge decision (superpowers:finishing-a-development-branch).

---

## Stitcher reconciliations (binding clarifications across task groups)

- **Feast threshold**: one shared `crate::tui::day::FEAST_DAY_RATIO` (added in
  Task 5, consumed by morning-after and Task 10 resonance). `MORNING_IDLE_RATIO`
  stays speech-local.
- **Codex lamp arm**: Task 10a (stitcher addendum) supplies
  `DayContext.yesterday_codex_share`; Task 10's lamp arm matches on it —
  recurring, per the spec — not on fresh-unlock.
- **`local_day_started_utc`**: owned by Task 4. Task 7's grep-guarded edits
  will find it present and must skip.
- **`USAGE_DISCONTINUITY_CODE`**: declared once in Task 2
  (`src/game/runtime.rs`); every later reference (surfacing exemptions,
  fixtures, smoke checks) uses the constant, never an inline string.
- **Refusal narrative**: one event per refused poll pass (not per surface);
  diagnostics stay per-surface. The event stamps `last_idle_narration_at`.
- **AnimationFrame/BreathRhythm producers**: every call site (including the
  Task 11 preview restamp) goes through `blink_slowdown_for_tiredness` and
  `breath_rhythm_for_day` (Task 6) — never re-derive the mapping inline.
- **Snapshot acceptance order**: snapshots are inspected-and-accepted in task
  order (Task 5 speech-line diffs → Task 7 motes → Task 8 sky family → Task
  11 new fixtures). Each accept step's "any other diff is a defect" guard is
  load-bearing; do not batch-accept.
- **`apply_usage_poll` test wrapper**: keeps its 4-param signature, passing
  `DISCONTINUITY_GUARD_RATIO` internally (config threading matters only on
  the two production paths).
- **`dawn-after-feast-day` preview fixture** (named in the spec's testing
  list): intentionally superseded — `light-day-morning` covers the
  morning-after channel and `heavy-day-evening` covers the feast-day
  visuals; Task 12's spec walk should treat it as covered, not missing.
- **Explicit supersession flagged for Drew**: Task 5 removes the legacy
  raw-token munch path (`recent_activity_tokens`, `RECENT_ACTIVITY_WINDOW`,
  and the test `recent_activity_tokens_uses_bucket_at_not_observed_at` in
  `src/commands/watch.rs`) because the vm build now routes speech through
  the live profile + DayContext. Replacement coverage lives in the Task 5
  speech tests. Approved at plan review.
- **Weekend wander softening** uses anchored time dilation: when live
  activity hard-wins over softening, the wander clock re-times in one
  bounded step coincident with the feed reaction. Accepted; revisit only if
  it reads badly in the preview fixtures.

## Appendix: the binding interface sheet

# T2+T3 Interface Sheet (binding for all plan authors)

Every type name, signature, and constant below is FIXED. If your task group
needs something not listed, use exactly what another group defines here — do
not invent parallel vocabulary. Spec: docs/superpowers/specs/2026-06-09-glorp-lives-in-time-design.md
(Amendment + Branch T2 + Branch T3 + Honesty rules + Testing sections).

## DayContext additions (src/tui/day.rs)

```rust
pub struct DayContext {
    // ... existing T1 fields unchanged ...
    /// Accumulated-active-time fatigue, 0.0..=1.0. Derived from the count of
    /// 10-minute buckets containing applied tokens in the trailing
    /// FATIGUE_WINDOW_HOURS, scaled by the window's volume ratio vs baseline.
    /// 0.0 while the maturity gate is closed.
    pub tiredness: f32,
    /// UTC instant the current local day began (motes tidy-fade anchor).
    pub local_day_started_utc: time::OffsetDateTime,
}
```

New pure helpers in day.rs:
```rust
/// Morning-after selection window: all of Dawn plus the first
/// MORNING_AFTER_DAY_MINUTES of Day. Pure function of carried instants.
pub fn in_morning_after_window(day: &DayContext, now: time::OffsetDateTime) -> bool;
/// The earned prop whose unlock provenance matches yesterday/climate
/// (heavy-session planter after a feast day, codex lamp after codex-heavy,
/// recovery sprout after wilt recovery); date_seed tie-breaks among equally
/// qualified; None when no signal qualifies. Ratio-qualified matches require
/// day.mature.
pub fn resonant_prop_for_day(
    day: &DayContext,
    earned: &[crate::storage::state::EarnedHabitatProp],
) -> Option<crate::game::habitat::HabitatPropId>;
/// Weekend softening factor 0.0 (none) ..= 1.0 (full), from is_weekend and
/// weekend_share: full at share <= WEEKEND_QUIET_SHARE, zero at
/// >= WEEKEND_ACTIVE_SHARE, linear between; 0.0 while immature.
pub fn weekend_softening(day: &DayContext) -> f32;
```

## Constants (all new, with the owning module)

| Constant | Value | Module |
|---|---|---|
| `DISCONTINUITY_GUARD_RATIO` | 5.0 (config-overridable: `discontinuity_guard_ratio` on AppConfig, serde default) | src/game/runtime.rs |
| `DISCONTINUITY_GUARD_FLOOR_TOKENS` | 50_000_000.0 | src/game/runtime.rs |
| `FATIGUE_WINDOW_HOURS` | 16 | src/tui/day.rs |
| `MORNING_AFTER_DAY_MINUTES` | 60 | src/tui/day.rs |
| `MOTE_TIDY_FADE_MINUTES` | 30 | src/tui/day.rs |
| `WEEKEND_QUIET_SHARE` | 0.10 | src/tui/day.rs |
| `WEEKEND_ACTIVE_SHARE` | 0.30 | src/tui/day.rs |
| `MOTE_BUDGET_SHARE` | 0.5 (of the ambient allocation) | src/tui/panels/pet.rs |
| `TIRED_BLINK_MAX_SLOWDOWN` | 24 (ticks added to blink cadence at tiredness 1.0) | src/pet/render.rs |
| `TIRED_BREATH_MAX_SCALE` | 1.5 (period multiplier at tiredness 1.0) | src/pet/animator.rs |
| `DREAM_WINDOW_MINUTES` | 10 (per dream window) | src/pet/speech.rs |

## Signature changes (binding)

```rust
// src/pet/speech.rs — REPLACES the bool-asleep variant from T1. Implements
// the full binding precedence stack from the spec (petting override stays
// app-side above this): asleep(dream window -> dream line by
// yesterday.dominant_shape, else zzz cadence) > live-burst munch > needy
// mood (Hungry/Sad/Wilted) > morning-after greeting flavor > default mood.
pub fn current_pet_speech_for_scene(
    mood: Mood,
    profile: &crate::tui::life::PetLifeProfile,
    day: &crate::tui::day::DayContext,
    now: OffsetDateTime,
) -> Option<String>;
```

```rust
// src/pet/render.rs — AnimationFrame stays Copy + Eq (integer fields only).
pub struct AnimationFrame {
    pub tick: u64,
    pub blink_suppression_ticks: u8,
    pub hold_eyes_closed: bool,
    /// Ticks added to the species blink cadence (tiredness slows blinking).
    /// 0 = normal. Producers map tiredness 0..1 -> 0..TIRED_BLINK_MAX_SLOWDOWN.
    pub blink_slowdown: u8,
}
```

```rust
// src/pet/animator.rs — new variant; existing variants unchanged.
pub enum BreathRhythm {
    Awake,
    Asleep { onset: time::OffsetDateTime },
    /// Lengthened period for a tired-but-awake pet. eighths in 0..=8 maps to
    /// period scale 1.0..=TIRED_BREATH_MAX_SCALE (integer to keep Copy+Eq).
    Tired { eighths: u8 },
}
```

```rust
// src/pet/activity.rs (or wherever format_hhmm lives) — local display fix.
// All EventView timestamp formatting goes through this; callers thread the
// offset (vm build: mapper.offset_at(now); install paths: LocalDayMapper::System).
pub fn format_hhmm_local(now: OffsetDateTime, offset: time::UtcOffset) -> String;
```

```rust
// src/game/runtime.rs — the guard lives INSIDE stage_usage_poll_deltas (the
// only chokepoint: status.rs has its own inline pipeline copy). Signature
// change (binding): the `baseline: CalibrationBaseline` param becomes
// `state: &mut PetState` (carries calibration + recent_events +
// last_idle_narration_at) and a `guard_ratio: f64` param is threaded from
// AppConfig by both poll paths:
pub fn stage_usage_poll_deltas(
    usage_store: &mut UsageStore,
    poll: &UsagePollResult,
    state: &mut PetState,
    guard_ratio: f64,
    now: OffsetDateTime,
) -> Result<Vec<i64>>;
// Per provider_surface: sum effective deltas; days_factor = whole days since
// usage_store.latest_cursor_updated_at(surface) + 1 (no cursors at all =>
// first contact => refuse); threshold = max(guard_ratio * baseline *
// days_factor, DISCONTINUITY_GUARD_FLOOR_TOKENS). Refused surface:
// usage_store.refuse_poll_discontinuity(its cursor_updates, &diagnostic, now)
// in ONE transaction, push NarrativeEvent "{name} declined an implausible
// feast" onto state.recent_events, stamp state.last_idle_narration_at =
// Some(now). Other surfaces stage normally.

// src/storage/usage_store.rs — new methods (binding):
pub fn latest_cursor_updated_at(
    &self,
    provider_surface: &str,
) -> crate::error::Result<Option<OffsetDateTime>>;
pub fn refuse_poll_discontinuity(
    &mut self,
    updates: Vec<ProviderCursorUpdate>,
    diagnostic: &ProviderDiagnostic,
    now: OffsetDateTime,
) -> crate::error::Result<()>; // cursor upserts + diagnostic insert, one tx
```

Surfacing exemptions (vm wiring group owns): the `usage_discontinuity`
diagnostic code is exempt from source_health's broken classification and from
active_diagnostics' ready-today filter; `glorp status` reads it from the
store, never claims "provider: blocked" for it.

```rust
// src/tui/panels/pet.rs — motes render in their own pass after ambient,
// before activity glyphs; same exclusion rules (silhouette halo + speech).
fn mote_glyphs_for(
    day: &crate::tui::day::DayContext,
    habitat: Rect,
    exclusions: &[Rect],
    now: time::OffsetDateTime,
    color_capability: crate::tui::style::ColorCapability,
) -> Vec<AmbientGlyph>;
// Density: soft-saturating (asymptotic, sub-countable) in today_ratio,
// capped at MOTE_BUDGET_SHARE of the ambient allocation, positions jittered
// by date_seed. During the first MOTE_TIDY_FADE_MINUTES after
// local_day_started_utc, render yesterday's density fading to zero
// (yesterday.ratio drives the fading set). Flat tier: zero motes (ambient
// contract unchanged). Gated on day.mature.
```

```rust
// src/tui/panels/pet.rs — T3 sky character + climate + seasons fold into the
// EXISTING T1 functions (extend, don't duplicate):
fn sky_palette_for_phase(species: Species, phase: DayPhase, date_seed: u64) -> &'static [char];
// date_seed picks among >=2 authored variants per (species, phase). Day
// family variants only — visual texture, never personality content.
fn sky_color_for_phase(phase: DayPhase, blend: f32, season: Season, climate: Option<WorkWeather>) -> Color;
// season: subtle hue drift only. climate: ambient tint bias; None/Clear = no bias.
```

## Wiring rules (who calls what)

- `install_poll_result` (src/tui/app.rs) and the vm build (src/commands/watch.rs)
  call `current_pet_speech_for_scene(mood, profile, &day_context, now)`.
  The petting override remains app-side and outranks everything.
- Breath call sites pick: asleep -> `Asleep{onset}`; else tiredness > 0.05 ->
  `Tired{eighths}`; else `Awake`.
- `rerender_pet_for_view_model` signature gains nothing new — it already takes
  `hold_eyes_closed`; `blink_slowdown` is computed by callers from
  `vm.day_context.tiredness` and passed via AnimationFrame at the SAME call
  sites T1 touched (watch build, app frame tick, menubar animate).
- Prop resonance consumes `resonant_prop_for_day` at the panel render
  (wander-target bias + gentle styling within existing caps), and is PAUSED
  while `day.asleep` (no glowing shrine over a sleeping pet).
- Weekend softening multiplies wander cadence + palette warmth; live-activity
  channels always win over softening.
- The guard runs in poll_usage_and_apply (src/commands/watch.rs) and the
  status poll path BEFORE stage_usage_poll_deltas — single chokepoint
  preferred if extraction confirms one exists; otherwise both call sites.

## Honesty invariants every task must respect

- No numbers/fill-direction/completion framing on motes; soft saturation only.
- Dreams render ONLY when yesterday has dominant_shape detail.
- Morning lines never reference the user's absence (authoring guardrail).
- Needy vitals (Hungry/Sad/Wilted) outrank every flavor channel.
- Flat tier: zero ambient/mote glyphs; timing cues only.
- Maturity gate governs every baseline-ratio channel (motes, tiredness,
  morning-after, ratio-qualified resonance, weekend softening).
- date_seed varies visual texture only — never speech/quirk content.

## Plan format (identical to the T1 plan)

Each task: `### Task N: <name>` + **Files:** (Create/Modify/Test with exact
paths) + checkbox steps: write failing test (with code) -> run red (command +
expected) -> implement (with code) -> run green -> fmt/clippy/commit (with
message). No placeholders, no "similar to Task N". Anchor warnings: Read the
target region before editing; cite path:line from the extraction contracts.
