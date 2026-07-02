# Glitch Metamorph Art Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the glitch species' six repeated-rectangle silhouettes (S1–S6) with the approved "digital metamorph" cast, restore living faces to the elders, shift the body color to magenta, and fix block-glyph mirroring so the facing flip reads.

**Architecture:** All art is data — seven `Template` constants per species in `src/pet/art.rs`, substituted and rendered by a content-agnostic renderer. This plan swaps six glitch templates, extends two small pure functions (`apply_interior_texture` pinning, `mirror_char` glyph pairs), and changes three palette constants. No renderer/pipeline changes. Every change is guarded by the existing invariant test suite plus new locking tests.

**Tech Stack:** Rust, `cargo test` (unit + integration), `cargo-insta` (`.snap` snapshots), `cargo clippy`/`cargo fmt`, the `dev-preview` command for visual review.

**Source spec:** `docs/superpowers/specs/2026-07-02-glitch-metamorph-art-design.md`

**Before starting:** create a WIP branch off `main` (e.g. `glitch-metamorph-art`). This plan targets `main`; it does not depend on the unmerged `private/glitch-persistent-corruption` branch (see *Coordination* at the end).

## Global Constraints

- Every art template is **exactly 8 rows × 11 display columns**. `{eyes}` occupies 3 display columns, `{mouth}` occupies 1. Enforced by `every_template_line_is_eleven_cells_wide` and `every_template_line_is_eleven_display_columns` (ambiguous-width = narrow) in `src/pet/art.rs`.
- Each stage's occupied-cell count must sit inside its `STAGE_CELL_BANDS` entry (`src/pet/art.rs:880`): S0 `[1,4]`, S1 `[5,10]`, S2 `[11,20]`, S3 `[21,34]`, S4 `[35,50]`, S5 `[51,66]`, S6 `[67,88]`. Occupied count = non-whitespace chars after `substitute_slots` (`{eyes}`→`"o o"` = 2 cells, `{mouth}`→`"w"` = 1 cell).
- Single-width glyphs only. The new templates introduce `╪ ═ ▪ · ◆` and the block/quadrant family; all verified width-1 under ambiguous=narrow. `◆ ═ ╪ ·` and the blocks are East-Asian-Ambiguous — the non-blocking `ambiguous_width_lint_warns_but_does_not_fail` test may list them; that is expected (fuzz and crystal already do). Do **not** add a `.is_empty()` assertion for glitch.
- Verify each task with the exit status of `cargo test`, never `cargo test | tail`. Run the **full** suite (not just `--lib`) — dev-preview snapshots and structural caps live in integration tests.
- The CI gate `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --check` must stay clean.
- Commit messages end with the `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` trailer.

---

### Task 1: Replace the six glitch silhouettes (S1–S6)

Swap the glitch templates for the metamorph cast. The key behavioral gains: S5/S6 regain `{mouth}` slots (elders keep a living face), and each stage is a distinct shape in-band. A new test locks the exact occupied-cell counts so a transcription typo is caught immediately.

**Files:**
- Modify: `src/pet/art.rs` — constants `GLITCH_S1`..`GLITCH_S6` (lines 495–554), the `glitch_base` doc comment (lines 119–123), and the stale comment inside `glitch_resting_face_is_alive` (line ~1316).
- Test: `src/pet/art.rs` test module — new `glitch_metamorph_cell_counts_and_face_slots`.

**Interfaces:**
- Produces: `GLITCH_S1`..`GLITCH_S6` `Template` constants with the new art; S2–S6 carry both `{eyes}` and `{mouth}` slots. Later tasks (2, 5) read these via `stage_base_template(Species::Glitch, stage)`.

- [ ] **Step 1: Write the failing lock test**

Add this test inside the `#[cfg(test)] mod tests` block in `src/pet/art.rs` (near `glitch_resting_face_is_alive`):

```rust
    #[test]
    fn glitch_metamorph_cell_counts_and_face_slots() {
        // Locks the metamorph silhouettes: exact occupied-cell counts per stage
        // (a transcription typo shifts these) and the elder face-slot restoration
        // (S5/S6 regain {mouth} so the daemon and kernel keep a living face).
        let counts = [
            (Stage::S1, 8),
            (Stage::S2, 20),
            (Stage::S3, 34),
            (Stage::S4, 48),
            (Stage::S5, 58),
            (Stage::S6, 70),
        ];
        for (stage, want) in counts {
            assert_eq!(
                rendered_occupied_cells(Species::Glitch, stage),
                want,
                "Glitch {stage:?} occupied cells"
            );
        }
        for stage in [Stage::S2, Stage::S3, Stage::S4, Stage::S5, Stage::S6] {
            let t = stage_base_template(Species::Glitch, stage)
                .iter()
                .copied()
                .collect::<String>();
            assert!(t.contains("{eyes}"), "Glitch {stage:?} needs an {{eyes}} slot");
            assert!(t.contains("{mouth}"), "Glitch {stage:?} needs a {{mouth}} slot");
        }
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib glitch_metamorph_cell_counts_and_face_slots`
Expected: FAIL — the old S1 count is 10 (not 8) and old S5/S6 have no `{mouth}` slot, so the assertions fire.

- [ ] **Step 3: Replace the six template constants**

In `src/pet/art.rs`, replace `GLITCH_S1` through `GLITCH_S6` (lines 495–554) with exactly these (leave `GLITCH_S0` at lines 485–494 unchanged):

```rust
const GLITCH_S1: Template = [
    "           ",
    "           ",
    "     ▀▜    ",
    "    ▐◉▌    ",
    "    ▘ ▝    ",
    "   ▪       ",
    "           ",
    "           ",
];
const GLITCH_S2: Template = [
    "           ",
    "  ▗▄▪▄ ·   ",
    "   ▌{eyes}▐   ",
    "   ▌░{mouth} ▐   ",
    "   ▙▄▄▄▟   ",
    "    ▘ ▞    ",
    "           ",
    "           ",
];
const GLITCH_S3: Template = [
    "  ▪·       ",
    " ▗▒▒▓▓▖    ",
    " ▒{eyes}▒▓    ",
    " ▒░{mouth}░▒▓    ",
    " ▝▓▓██▘╪▛▜ ",
    "  ▚▞    ▙▟·",
    "        :  ",
    "           ",
];
const GLITCH_S4: Template = [
    "     ▛▀▀▀▜ ",
    "   ·░▌▒▓▒▐ ",
    "  ▌░{eyes}░▐  ",
    "  ▌▒ {mouth} ▪▐  ",
    "▌▓█▒▓▐░·   ",
    "▌▒▓▓▒▐ ▖   ",
    "▙▄▄▄▄▟     ",
    " ▘▝  ▝▘    ",
];
const GLITCH_S5: Template = [
    " ▟▙     ▐  ",
    " ██▄▀▄▀▄▞  ",
    "▐░░{eyes}▒▓▌  ",
    " ▌▒▒{mouth}▒▒▓█▐ ",
    "▚▟▛◆▜▒▓█▙  ",
    " ▐▒▒▓▓███▟▘",
    "  ▜▓▓██▛   ",
    "  ▘▀ ▝█▘   ",
];
const GLITCH_S6: Template = [
    "▛▀▀▀▖  :░  ",
    "▌▓▛▀▀▀▜▒░  ",
    "▌▒▌{eyes}▐▓▒ ▜",
    "▌▓▌░{mouth}░▐▓▒ ▐",
    "▌▒▙▄▄▄▟▒██ ",
    "▌░▓═▓▓▒▓█ ▗",
    "▙▄▄▄▟▓▓▒░  ",
    " ▚▞  ▄▓▄ · ",
];
```

- [ ] **Step 4: Refresh the two stale comments**

Replace the `glitch_base` doc comment (lines 119–123) with:

```rust
// Validated Packet Daemon cast, "digital metamorph": each stage is a distinct
// digital object, not a scaled packet. The packet-box is the creature's
// childhood shell — popped lid (S2), towed frame-trailer (S3), askew slabs (S4),
// glowing chest-dock panel (S5), half-shed carapace (S6). Living lens eyes via
// {eyes}; {mouth} slot wired at S2–S6 so every stage including the elders keeps
// a living face. Validated cell ramp [3,8,20,34,48,58,70], strictly increasing.
```

In `glitch_resting_face_is_alive`, replace the stale S4 assertion message (line ~1316) — S4 is no longer a "closed packet":

```rust
            "Glitch S4 must render a dense glitching body, got:\n{art}"
```

- [ ] **Step 5: Run the lock test and the full art invariant suite**

Run: `cargo test --lib -- pet::art`
Expected: PASS — including `glitch_metamorph_cell_counts_and_face_slots`, `glitch_base_art_passes_phase1_invariants`, `every_template_line_is_eleven_cells_wide`, `every_template_line_is_eleven_display_columns`, `s6_fills_art_rows_for_every_species`, and `glitch_resting_face_is_alive`.

- [ ] **Step 6: Run the full suite and review snapshot churn**

Run: `cargo test`
Expected: PASS, but `cargo-insta` may report changed `.snap` frames (any fixture that renders a glitch pet, e.g. `dev_preview__watch_wide_normal_frame` if its pet is glitch, and the glitch dev-preview `pets` fixtures). Review each: `cargo insta review`. Accept **only** diffs that are the new glitch art; reject anything else and investigate. If no snapshots changed, skip.

- [ ] **Step 7: Commit**

```bash
git add src/pet/art.rs tests/snapshots
git commit -m "feat(pet): metamorph glitch silhouettes S1-S6

Replace the repeated-rectangle glitch art with the digital-metamorph
cast (box as childhood shell); restore {mouth} slots to S5/S6 so the
elders keep a living face. New cell ramp [3,8,20,34,48,58,70].

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: Pin elder interior texture per (species, stage)

The new S5/S6 bodies are hand-shaded (directional `░▒▓█` gradient, light from upper-left). The per-seed `▒↔▓` swap in `apply_interior_texture` would scramble that into checkerboard noise — the same reason crystal is pinned. Pin glitch S5/S6; keep glitch S3/S4 unpinned so their static bodies still carry on-brand per-individual noise.

**Files:**
- Modify: `src/pet/art.rs` — `apply_interior_texture` pin condition (lines 189–190).
- Test: `src/pet/art.rs` test module — new `glitch_elder_texture_is_pinned_but_mid_varies`.

**Interfaces:**
- Consumes: the Task 1 templates via `stage_base_template`.

- [ ] **Step 1: Write the failing test**

Add to the test module in `src/pet/art.rs`:

```rust
    #[test]
    fn glitch_elder_texture_is_pinned_but_mid_varies() {
        // S5/S6 are hand-shaded, so their interior texture must be pinned (crystal
        // lesson). S3/S4 static bodies keep per-seed variety.
        let base_s5 = stage_base_template(Species::Glitch, Stage::S5);
        let a = apply_interior_texture(base_s5, Species::Glitch, Stage::S5, 1);
        let b = apply_interior_texture(base_s5, Species::Glitch, Stage::S5, 999);
        assert_eq!(a, b, "Glitch S5 is hand-shaded: interior texture must be pinned");

        let base_s6 = stage_base_template(Species::Glitch, Stage::S6);
        let c = apply_interior_texture(base_s6, Species::Glitch, Stage::S6, 1);
        let d = apply_interior_texture(base_s6, Species::Glitch, Stage::S6, 999);
        assert_eq!(c, d, "Glitch S6 is hand-shaded: interior texture must be pinned");

        let base_s4 = stage_base_template(Species::Glitch, Stage::S4);
        let variants: std::collections::HashSet<_> = (0..8u64)
            .map(|s| apply_interior_texture(base_s4, Species::Glitch, Stage::S4, s))
            .collect();
        assert!(variants.len() > 1, "Glitch S4 must keep per-seed interior variety");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib glitch_elder_texture_is_pinned_but_mid_varies`
Expected: FAIL — glitch S5/S6 are currently unpinned, so seeds 1 and 999 produce different interiors (`assert_eq` fires).

- [ ] **Step 3: Extend the pin condition**

In `src/pet/art.rs`, replace the `pinned` binding in `apply_interior_texture` (lines 189–190):

```rust
    let pinned = matches!(stage, Stage::S0 | Stage::S1 | Stage::S2)
        || matches!(species, Species::Crystal)
        || matches!(
            (species, stage),
            (Species::Glitch, Stage::S5) | (Species::Glitch, Stage::S6)
        );
```

Update the doc comment above the function (line 179) to append: `Glitch S5/S6 pinned (hand-shaded elders).`

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --lib glitch_elder_texture_is_pinned_but_mid_varies`
Expected: PASS.

- [ ] **Step 5: Run the art suite**

Run: `cargo test --lib -- pet::art`
Expected: PASS (band counts are computed at `REFERENCE_SEED` and unchanged by pinning).

- [ ] **Step 6: Commit**

```bash
git add src/pet/art.rs
git commit -m "feat(pet): pin glitch S5/S6 interior texture

Hand-shaded elder bodies would scramble under the per-seed swap
(crystal lesson); S3/S4 stay unpinned for on-brand static variety.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: Fix block-glyph mirroring for the facing flip

When a pet turns left (`vm.facing == -1`), `mirror_line` reverses each art line and swaps directional glyphs via `mirror_char`. Today it only swaps `()/\<>[]{}bd` — block quadrants pass through unswapped, so a block-built silhouette (the new glitch art, and blob's `▟▙` shoulders) reverses position but keeps the wrong handedness. Add the six block/quadrant mirror pairs. This corrects every species' left-facing render, so review the visual diff across species, not just glitch.

**Files:**
- Modify: `src/tui/panels/pet/art_lines.rs` — `mirror_char` (lines 239–255).
- Test: `src/tui/panels/pet/art_lines.rs` test module (`mod tests` at line 454) — new `mirror_line_swaps_block_quadrant_pairs`.

- [ ] **Step 1: Write the failing test**

Add to the `mod tests` block in `src/tui/panels/pet/art_lines.rs`:

```rust
    #[test]
    fn mirror_line_swaps_block_quadrant_pairs() {
        // Block-built silhouettes must flip glyph handedness when the pet turns,
        // not merely reverse position. Reverse + swap: "▛▙▌▖▘▚" -> "▞▝▗▐▟▜".
        assert_eq!(mirror_line("▛▙▌▖▘▚"), "▞▝▗▐▟▜");
        // Each pair is an involution, so mirroring twice is identity.
        let s = "▟▒▒▙▐░▌";
        assert_eq!(mirror_line(&mirror_line(s)), s);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib mirror_line_swaps_block_quadrant_pairs`
Expected: FAIL — `mirror_line("▛▙▌▖▘▚")` currently returns `"▚▘▖▌▙▛"` (reversed only), not `"▞▝▗▐▟▜"`.

- [ ] **Step 3: Add the block-pair arms**

In `src/tui/panels/pet/art_lines.rs`, add these arms to the `match c` in `mirror_char`, immediately before `_ => c,`:

```rust
        '\u{259B}' => '\u{259C}', // ▛ <-> ▜
        '\u{259C}' => '\u{259B}',
        '\u{2599}' => '\u{259F}', // ▙ <-> ▟
        '\u{259F}' => '\u{2599}',
        '\u{258C}' => '\u{2590}', // ▌ <-> ▐
        '\u{2590}' => '\u{258C}',
        '\u{2596}' => '\u{2597}', // ▖ <-> ▗
        '\u{2597}' => '\u{2596}',
        '\u{2598}' => '\u{259D}', // ▘ <-> ▝
        '\u{259D}' => '\u{2598}',
        '\u{259A}' => '\u{259E}', // ▚ <-> ▞
        '\u{259E}' => '\u{259A}',
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --lib mirror_line_swaps_block_quadrant_pairs`
Expected: PASS.

- [ ] **Step 5: Run the full suite and review cross-species snapshot churn**

Run: `cargo test`
Expected: PASS. Any snapshot capturing a **left-facing** pet of **any** species may churn (blob shoulders now flip correctly, etc.). Review with `cargo insta review`; confirm each diff is corrected mirror handedness (a left-facing silhouette that now reads as a proper turn), and accept. Investigate anything that looks broken rather than merely flipped.

- [ ] **Step 6: Commit**

```bash
git add src/tui/panels/pet/art_lines.rs tests/snapshots
git commit -m "fix(pet): mirror block-quadrant glyphs on facing flip

mirror_char only swapped ()/\\<>[]{} — block-built silhouettes kept
the wrong handedness when facing left. Add the 6 quadrant/half-block
pairs so glitch (and blob's shoulders) turn correctly.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Shift glitch color identity to magenta

Glitch's acid-green body (hue 135°) sat only 15° from blob's mint (150°), and the ±18° per-pet jitter made the ranges overlap; the fixed acid-green corruption color camouflaged on glitch itself. Move the body hue to magenta (345°). Corruption stays acid green (now a contrasting spark), the resting eye becomes spring green (complementary), and the feed-pulse color follows the identity.

**Files:**
- Modify: `src/pet/palette.rs` — `species_base_hue` glitch arm (line 163), the comment in `species_body_chroma` (line 171), and the test `species_base_hues_match_identity_family` (lines 553, 558).
- Modify: `src/tui/panels/pet/colors.rs` — the test `activity_lift_does_not_invert_body_hue_at_high_chroma` (lines 376–410).
- Modify: `src/pet/animator.rs` — `species_feed_color` glitch arm (line 387).

- [ ] **Step 1: Update the hue-family test to expect magenta (failing)**

In `src/pet/palette.rs`, in `species_base_hues_match_identity_family`, change the glitch assertion (line 558) and its comment (line 553):

```rust
        // OKLCH hue degrees (approx): peach ~40, mint ~150, lavender ~300,
        // magenta ~345, ice ~230, amber ~75. Verify the family anchor, not exact
```
```rust
        assert!((species_base_hue(Species::Glitch) - 345.0).abs() < 1.0);
```

- [ ] **Step 2: Rewrite the activity-lift hue-read test for magenta (failing)**

In `src/tui/panels/pet/colors.rs`, replace the body of `activity_lift_does_not_invert_body_hue_at_high_chroma` (the comment at 379–380 and the two assertion blocks). Magenta body resolves to `(247,118,196)`; a hard lift adds 44 per channel saturating → `(255,162,240)`. Both keep red and blue over green:

```rust
        // The loudest body (Glitch magenta). Lift it hard and confirm the magenta
        // hue read survives (red and blue stay over green; no channel pins to 255
        // and flips the read).
```

Replace the pre-lift assertion:

```rust
        let body_before = palette.body;
        assert!(
            body_before.r >= body_before.g && body_before.b >= body_before.g,
            "glitch body should read magenta (r,b over g) before lift: {body_before:?}"
        );
```

Replace the post-lift assertion:

```rust
        if let Some(ratatui::style::Color::Rgb(r, g, b)) = lifted.fg {
            assert!(
                r >= g && b >= g,
                "max activity lift must not flip glitch body off magenta: ({r},{g},{b})"
            );
        } else {
            panic!("lifted body should stay Rgb");
        }
```

- [ ] **Step 3: Run both tests to verify they fail**

Run: `cargo test --lib species_base_hues_match_identity_family activity_lift_does_not_invert_body_hue_at_high_chroma`
Expected: FAIL — `species_base_hue(Glitch)` still returns 135.0, and the magenta assertions fail against the still-green body.

- [ ] **Step 4: Shift the body hue and feed color**

In `src/pet/palette.rs`, `species_base_hue` (line 163):

```rust
        Species::Glitch => 345.0,  // magenta/hot-pink daemon
```

In `src/pet/palette.rs`, the `species_body_chroma` doc comment (line 171), change `(Glitch acid, Mech amber)` to `(Glitch magenta, Mech amber)`. Leave the chroma value at `0.18`.

In `src/pet/animator.rs`, `species_feed_color` (line 387):

```rust
        Some(Species::Glitch) => Color::Rgb(255, 140, 215), // hot magenta
```

- [ ] **Step 5: Run the palette + color + animator tests to verify they pass**

Run: `cargo test --lib -- palette:: animator::tests colors::tests`
Expected: PASS — including the two rewritten tests, plus `corruption_role_resolves_to_a_contrasting_acid_color` (corruption is still acid green and now contrasts a magenta body), `resting_eye_stays_hue_distinct_from_body_across_seeds`, `all_species_bodies_are_mutually_distinct`, and `feed_color_differs_per_species` (magenta still differs from fuzz/mech).

- [ ] **Step 6: Commit**

```bash
git add src/pet/palette.rs src/tui/panels/pet/colors.rs src/pet/animator.rs
git commit -m "feat(pet): shift glitch identity to magenta

Acid green sat 15 deg from blob mint with overlapping jitter, and the
fixed acid-green corruption camouflaged on glitch itself. Magenta 345
raises the min hue gap to 45 deg and makes corruption sparks pop.
Feed-pulse color follows the identity.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: Full verification, preview lab, and visual review

The final gate: prove the whole suite is green under the CI settings, regenerate the preview bundle, and confirm the art reads as the spec intends across the stage matrix and the facing flip.

**Files:** none (verification + review).

- [ ] **Step 1: Full test suite**

Run: `cargo test`
Expected: PASS, exit 0. Confirm no unreviewed snapshot changes remain (`git status` clean except intended commits).

- [ ] **Step 2: Formatting and lint gates**

Run: `cargo fmt --check`
Expected: no output, exit 0.

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: no warnings, exit 0.

- [ ] **Step 3: Regenerate the preview bundle**

Run: `cargo run -- dev-preview --scenario pets --out target/glorp-preview-glitch`
Expected: exit 0, bundle written.

- [ ] **Step 4: Visual review (manual gate)**

Open `target/glorp-preview-glitch/index.html`. Confirm on the `pet-species-stage` and `pet-glitch-live-states` frames:
- Each glitch stage S1–S6 reads as a distinct shape (capsule-bit, lid-ajar packet, chip-and-caboose, zigzag wafer, coredock imp, shed carapace) — not one rectangle scaled.
- The face (lens eyes + mouth) reads clearly at every stage, elders included.
- The body renders **magenta**, corruption ticks read as contrasting acid-green, the resting eye reads spring-green.
- Compare against `src/pet/art.rs` glitch templates for transcription fidelity.

Also generate the watch scenario and eyeball a left-facing frame to confirm the flip reads:
Run: `cargo run -- dev-preview --scenario watch --out target/glorp-preview-watch`
Open its `index.html`; find a facing/liveliness fixture and confirm the turned silhouette reads as the same creature rotated, for glitch **and** blob.

If any stage disappoints, the spec's Appendix records vetted alternates (S5 buttress gargoyle, S6 reactor porthole) and the full 22-candidate archive is at `.superpowers/brainstorm/70621-1783016793/glitch-art-candidates-all22.json`; swapping one is a template-only change that re-runs Task 1's tests.

- [ ] **Step 5: Companion note (do not block on this)**

The macOS companion renders from a separately-built app bundle. On-device confirmation needs `node scripts/build-macos-companion-app.mjs` + reopen; the shared render seam means the new art/color/mirror already flow to it. This is a Drew-side visual check, not a plan gate.

- [ ] **Step 6: Final commit (if the preview review prompted any template tweak)**

If Step 4 required a template edit, re-run Task 1 Steps 5–6 and commit. Otherwise nothing to commit here — the work landed in Tasks 1–4.

---

## Coordination with the `private/glitch-persistent-corruption` branch

This plan targets `main` and is self-contained: `src/pet/art.rs` is **untouched** on that branch, so the template work will not conflict. The one interaction is the persistence branch's hardcoded S5/S6 "protected expression island" in `is_protected_glitch_face_cell` — it exists only because the old elders had no `{mouth}` slot. Once these new templates land, the elders carry real `{eyes}`/`{mouth}` spans, so the generic Eye/Mouth-span protection covers them and the island becomes dead code.

**Whichever branch merges second** owns that cleanup:
- If this art work merges first, the persistence branch (when it rebases) re-derives `safe_glitch_patch_candidates` against the new templates and deletes its island.
- If the persistence branch merges first, add a follow-up commit here deleting the island.

Deferred from this plan (unchanged scope): a second "glow" body-shade role, habitat/room dialect, particle families, other species' palettes and art.

## Self-Review Notes

- **Spec coverage:** seven templates → Task 1; per-(species,stage) texture pinning → Task 2; `mirror_char` block pairs → Task 3; magenta hue + derived roles → Task 4; glyph audit + test updates + preview gate → Tasks 1/4/5. Elder `{mouth}` restoration → Task 1 (asserted). All spec "Renderer changes" 1–4 mapped.
- **Cell counts** were re-validated against `STAGE_CELL_BANDS` and the `substitute_slots` convention: S1–S6 = 8/20/34/48/58/70, all in-band, strictly increasing from S0=3.
- **Glyph widths** re-checked: every new glyph is width-1 under ambiguous=narrow (the display-column test's mode); ambiguous glyphs surface only in the non-blocking lint.
