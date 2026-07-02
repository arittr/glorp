# Glorp Glitch Persistent Corruption Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the Glitch species deterministic day-local repair marks plus bounded session glitch-outs, with Preview Lab proof across pet, watch, and round surfaces.

**Architecture:** Add a discrete Glitch render contract to `AnimationFrame`, derive day-local patch positions from pet seed/date seed/stage only, and render repaired marks by mutating glyphs plus splitting spans. Watch builds the presentation input after day/activity context exists and rerenders through `rerender_pet_for_view_model`, so the round companion inherits the same art. Preview Lab records the deterministic inputs in manifest metadata and provides the visual review gate.

**Tech Stack:** Rust, Ratatui buffers/layouts, `unicode-width`, serde preview manifests, existing hidden `dev-preview` command, Cargo unit and integration tests.

**Spec:** `docs/superpowers/specs/2026-07-02-glitch-persistent-corruption-design.md`

## Global Constraints

- No new persisted pet identity fields.
- No `state.json` schema migration.
- No provider- or harness-branded Glitch behavior.
- No reads from `source_accent`, `source_diversity`, provider names, display names, or provider-first-use props when deriving Glitch corruption.
- No high-frequency flicker that makes the pet unreadable.
- No render-time stage-up trigger in v1.
- No change to prop IDs, unlock semantics, calibration, ledger storage, or activity identity derivation.
- Patch positions are based on pet seed, `DayContext.date_seed`, `Species::Glitch`, and stage only.
- Patch count is based on `GlitchPatchTier`.
- Runtime patch glyphs must be display-width 1 under `unicode_width`.
- Persistent repaired marks must not use `PaletteRoleName::Corruption`.
- Existing TUI role spans must stay sorted and non-overlapping.

---

## File Structure

| File | Responsibility |
|---|---|
| `src/pet/render.rs` | Own Glitch corruption contract types, day-tier quantization, safe-cell selection, repaired mark rendering, transient burst rendering, and unit tests. |
| `src/commands/watch.rs` | Build `GlitchCorruptionFrame` from `WatchViewModel` data after day/activity context exists, then rerender the pet once through the existing adapter. |
| `src/dev_preview/pets.rs` | Add deterministic `pet-glitch-persistence-states` review frame and Glitch persistence inputs. |
| `src/dev_preview/watch.rs` | Add watch fixtures for patched quiet, patched active, burst, and calm-hot Glitch states. |
| `src/dev_preview/round.rs` | Add `round-glitch-patched-s6`, using the same rerendered `WatchViewModel.pet_art` path. |
| `src/dev_preview/scenarios.rs` | Emit manifest inputs for Glitch patch frames: date seed, tier, burst level, calm mode, feed reaction, selected cells, protected cells, expected count, and reset/restart flags. |
| `tests/dev_preview.rs` | Assert Preview Lab files and manifest inputs for Glitch persistence frames. |
| `tests/round_scene.rs` | Assert the round Glitch S6 fixture keeps at least one declared patch cell visible inside the safe aperture. |
| `tests/generation.rs`, `src/tui/layout.rs`, `src/tui/panels/pet.rs` | Update explicit `AnimationFrame` literals if adding the Glitch field makes them non-exhaustive. |

---

### Task 1: Add the Discrete Glitch Render Contract

**Files:**
- Modify: `src/pet/render.rs`
- Modify: every explicit `AnimationFrame { ... }` literal reported by `rg -n "AnimationFrame \\{" src tests`
- Test: `src/pet/render.rs`

**Interfaces:**
- Produces: `GlitchCorruptionFrame`, `GlitchPatchTier`, `GlitchBurstLevel`, `glitch_corruption_frame_for_inputs`.
- Consumes later: Task 2 reads `GlitchPatchTier::max_marks`; Task 4 calls `glitch_corruption_frame_for_inputs`.

- [ ] **Step 1: Write failing contract tests**

Add these tests inside `#[cfg(test)] mod tests` in `src/pet/render.rs`:

```rust
#[test]
fn glitch_patch_tier_quantizes_today_ratio_without_live_activity() {
    assert_eq!(GlitchPatchTier::from_today_ratio(-1.0), GlitchPatchTier::Quiet);
    assert_eq!(GlitchPatchTier::from_today_ratio(f32::NAN), GlitchPatchTier::Quiet);
    assert_eq!(GlitchPatchTier::from_today_ratio(0.0), GlitchPatchTier::Quiet);
    assert_eq!(GlitchPatchTier::from_today_ratio(0.74), GlitchPatchTier::Quiet);
    assert_eq!(GlitchPatchTier::from_today_ratio(0.75), GlitchPatchTier::Active);
    assert_eq!(GlitchPatchTier::from_today_ratio(1.49), GlitchPatchTier::Active);
    assert_eq!(GlitchPatchTier::from_today_ratio(1.5), GlitchPatchTier::Heavy);
    assert_eq!(GlitchPatchTier::Pristine.max_marks(), 0);
    assert_eq!(GlitchPatchTier::Quiet.max_marks(), 1);
    assert_eq!(GlitchPatchTier::Active.max_marks(), 2);
    assert_eq!(GlitchPatchTier::Heavy.max_marks(), 3);
}

#[test]
fn glitch_burst_level_quantizes_live_burst_for_eq_animation_frame() {
    assert_eq!(GlitchBurstLevel::from_burst_level(-1.0), GlitchBurstLevel::None);
    assert_eq!(GlitchBurstLevel::from_burst_level(f32::NAN), GlitchBurstLevel::None);
    assert_eq!(GlitchBurstLevel::from_burst_level(0.2), GlitchBurstLevel::None);
    assert_eq!(GlitchBurstLevel::from_burst_level(0.21), GlitchBurstLevel::Small);
    assert_eq!(GlitchBurstLevel::from_burst_level(0.69), GlitchBurstLevel::Small);
    assert_eq!(GlitchBurstLevel::from_burst_level(0.7), GlitchBurstLevel::Strong);
}

#[test]
fn glitch_corruption_frame_keeps_patch_inputs_separate_from_live_inputs() {
    let frame = glitch_corruption_frame_for_inputs(42, 1.6, 0.8, true, false);

    assert_eq!(frame.day_seed, 42);
    assert_eq!(frame.patch_tier, GlitchPatchTier::Heavy);
    assert_eq!(frame.burst_level, GlitchBurstLevel::Strong);
    assert!(frame.calm_mode);
    assert!(!frame.feed_reaction);
}
```

- [ ] **Step 2: Run tests and verify the missing types**

Run:

```bash
cargo test --lib pet::render::tests::glitch_patch_tier_quantizes_today_ratio_without_live_activity
cargo test --lib pet::render::tests::glitch_burst_level_quantizes_live_burst_for_eq_animation_frame
cargo test --lib pet::render::tests::glitch_corruption_frame_keeps_patch_inputs_separate_from_live_inputs
```

Expected: FAIL with missing `GlitchPatchTier`, `GlitchBurstLevel`, and `glitch_corruption_frame_for_inputs`.

- [ ] **Step 3: Add the contract types**

Add this code near `AnimationFrame` in `src/pet/render.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GlitchCorruptionFrame {
    pub day_seed: u64,
    pub patch_tier: GlitchPatchTier,
    pub burst_level: GlitchBurstLevel,
    pub calm_mode: bool,
    pub feed_reaction: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlitchPatchTier {
    Pristine,
    #[default]
    Quiet,
    Active,
    Heavy,
}

impl GlitchPatchTier {
    pub const fn max_marks(self) -> usize {
        match self {
            Self::Pristine => 0,
            Self::Quiet => 1,
            Self::Active => 2,
            Self::Heavy => 3,
        }
    }

    pub fn from_today_ratio(today_ratio: f32) -> Self {
        if !today_ratio.is_finite() {
            return Self::Quiet;
        }
        if today_ratio >= 1.5 {
            Self::Heavy
        } else if today_ratio >= 0.75 {
            Self::Active
        } else {
            Self::Quiet
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pristine => "pristine",
            Self::Quiet => "quiet",
            Self::Active => "active",
            Self::Heavy => "heavy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GlitchBurstLevel {
    #[default]
    None,
    Small,
    Strong,
}

impl GlitchBurstLevel {
    pub fn from_burst_level(burst_level: f32) -> Self {
        if !burst_level.is_finite() || burst_level <= 0.2 {
            Self::None
        } else if burst_level < 0.7 {
            Self::Small
        } else {
            Self::Strong
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Small => "small",
            Self::Strong => "strong",
        }
    }
}

pub fn glitch_corruption_frame_for_inputs(
    day_seed: u64,
    today_ratio: f32,
    burst_level: f32,
    calm_mode: bool,
    feed_reaction: bool,
) -> GlitchCorruptionFrame {
    GlitchCorruptionFrame {
        day_seed,
        patch_tier: GlitchPatchTier::from_today_ratio(today_ratio),
        burst_level: GlitchBurstLevel::from_burst_level(burst_level),
        calm_mode,
        feed_reaction,
    }
}
```

- [ ] **Step 4: Add the optional field to `AnimationFrame`**

Modify `AnimationFrame`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimationFrame {
    pub tick: u64,
    pub blink_suppression_ticks: u8,
    pub hold_eyes_closed: bool,
    pub blink_slowdown: u8,
    pub soft_eyes: bool,
    pub work_accent: WorkAccent,
    pub feed_reaction: bool,
    pub glitch_corruption: Option<GlitchCorruptionFrame>,
}
```

For explicit literals, prefer the local pattern:

```rust
AnimationFrame {
    tick,
    hold_eyes_closed,
    blink_slowdown,
    soft_eyes,
    work_accent,
    feed_reaction,
    ..AnimationFrame::default()
}
```

- [ ] **Step 5: Run contract tests and compile affected literals**

Run:

```bash
cargo test --lib pet::render::tests::glitch_patch_tier_quantizes_today_ratio_without_live_activity
cargo test --lib pet::render::tests::glitch_burst_level_quantizes_live_burst_for_eq_animation_frame
cargo test --lib pet::render::tests::glitch_corruption_frame_keeps_patch_inputs_separate_from_live_inputs
cargo check
```

Expected: PASS and `cargo check` exits 0.

- [ ] **Step 6: Commit**

```bash
git add src/pet/render.rs src/commands/watch.rs src/dev_preview/pets.rs src/tui/layout.rs src/tui/panels/pet.rs tests/generation.rs
git commit -m "feat(pet): add glitch corruption render contract"
```

---

### Task 2: Implement Safe Day-Local Patch Selection

**Files:**
- Modify: `src/pet/render.rs`
- Test: `src/pet/render.rs`

**Interfaces:**
- Consumes: `GlitchPatchTier::max_marks` from Task 1.
- Produces: `GlitchPatchCell`, `safe_glitch_patch_candidates`, `is_protected_glitch_face_cell`, `ordered_glitch_patch_cells`, `selected_glitch_patch_cells`.

- [ ] **Step 1: Write failing selector tests**

Add this helper and these tests inside `src/pet/render.rs`:

```rust
fn raw_glitch_art_for_test(
    seed: &str,
    stage: Stage,
) -> (GeneratedPet, Vec<String>, Vec<StyledSegment>) {
    let pet = generate_pet(seed).with_species(Species::Glitch);
    let expression = expression_for(&pet, Mood::Content, false, AnimationFrame::default());
    let raw = stage_template_lines(Species::Glitch, stage, u64::from(pet.traits.seed_hue));
    let rendered = raw
        .iter()
        .enumerate()
        .map(|(line_index, line)| render_template_line(line, line_index, &pet, &expression))
        .collect::<Vec<_>>();
    let lines = rendered.iter().map(|line| line.text.clone()).collect::<Vec<_>>();
    let spans = rendered.into_iter().flat_map(|line| line.spans).collect::<Vec<_>>();
    (pet, lines, spans)
}

#[test]
fn glitch_safe_patch_candidates_exclude_face_and_outline() {
    let (_pet, lines, spans) = raw_glitch_art_for_test("glitch-safe-candidates", Stage::S4);
    let candidates = safe_glitch_patch_candidates(Stage::S4, &lines, &spans);

    assert!(candidates.contains(&GlitchPatchCell { row: 3, col: 5 }));
    assert!(!candidates.contains(&GlitchPatchCell { row: 1, col: 5 }), "eye row is protected");
    assert!(!candidates.contains(&GlitchPatchCell { row: 2, col: 5 }), "mouth row is protected");
    assert!(!candidates.contains(&GlitchPatchCell { row: 0, col: 1 }), "top outline is protected");
}

#[test]
fn glitch_elder_expression_island_is_protected() {
    let spans = vec![StyledSegment {
        line: 1,
        start: 4,
        end: 7,
        role: PaletteRoleName::Eye,
    }];

    assert!(is_protected_glitch_face_cell(Stage::S5, 1, 5, &spans));
    assert!(is_protected_glitch_face_cell(Stage::S5, 2, 5, &spans));
    assert!(is_protected_glitch_face_cell(Stage::S6, 3, 7, &spans));
    assert!(!is_protected_glitch_face_cell(Stage::S6, 4, 5, &spans));
}

#[test]
fn ordered_glitch_patch_cells_are_stable_and_day_local() {
    let (pet, lines, spans) = raw_glitch_art_for_test("glitch-ordered-patches", Stage::S4);

    let first = ordered_glitch_patch_cells(&pet, Stage::S4, 123, &lines, &spans);
    let second = ordered_glitch_patch_cells(&pet, Stage::S4, 123, &lines, &spans);
    let next_day = ordered_glitch_patch_cells(&pet, Stage::S4, 124, &lines, &spans);

    assert_eq!(first, second);
    assert_ne!(first, next_day);
    assert!(first.len() >= 3, "S4 should have enough safe cells for a heavy day");
}

#[test]
fn tier_selection_reveals_prefix_without_relocating_marks() {
    let (pet, lines, spans) = raw_glitch_art_for_test("glitch-tier-prefix", Stage::S4);

    let quiet = selected_glitch_patch_cells(
        &pet,
        Stage::S4,
        555,
        GlitchPatchTier::Quiet,
        &lines,
        &spans,
    );
    let active = selected_glitch_patch_cells(
        &pet,
        Stage::S4,
        555,
        GlitchPatchTier::Active,
        &lines,
        &spans,
    );
    let heavy = selected_glitch_patch_cells(
        &pet,
        Stage::S4,
        555,
        GlitchPatchTier::Heavy,
        &lines,
        &spans,
    );

    assert_eq!(&active[..quiet.len()], quiet.as_slice());
    assert_eq!(&heavy[..active.len()], active.as_slice());
}
```

- [ ] **Step 2: Run selector tests and verify failure**

Run:

```bash
cargo test --lib pet::render::tests::glitch_safe_patch_candidates_exclude_face_and_outline
cargo test --lib pet::render::tests::glitch_elder_expression_island_is_protected
cargo test --lib pet::render::tests::ordered_glitch_patch_cells_are_stable_and_day_local
cargo test --lib pet::render::tests::tier_selection_reveals_prefix_without_relocating_marks
```

Expected: FAIL with missing selector symbols.

- [ ] **Step 3: Add cell type, allowlist, and safety helpers**

Add this code near the current corruption helpers in `src/pet/render.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlitchPatchCell {
    pub row: usize,
    pub col: usize,
}

const GLITCH_S3_PATCH_CELLS: &[GlitchPatchCell] = &[
    GlitchPatchCell { row: 4, col: 4 },
    GlitchPatchCell { row: 4, col: 5 },
    GlitchPatchCell { row: 4, col: 6 },
];

const GLITCH_S4_PATCH_CELLS: &[GlitchPatchCell] = &[
    GlitchPatchCell { row: 3, col: 4 },
    GlitchPatchCell { row: 3, col: 5 },
    GlitchPatchCell { row: 3, col: 6 },
    GlitchPatchCell { row: 4, col: 4 },
    GlitchPatchCell { row: 4, col: 5 },
    GlitchPatchCell { row: 4, col: 6 },
];

const GLITCH_S5_PATCH_CELLS: &[GlitchPatchCell] = &[
    GlitchPatchCell { row: 4, col: 3 },
    GlitchPatchCell { row: 4, col: 4 },
    GlitchPatchCell { row: 4, col: 5 },
    GlitchPatchCell { row: 4, col: 6 },
    GlitchPatchCell { row: 4, col: 7 },
    GlitchPatchCell { row: 5, col: 3 },
    GlitchPatchCell { row: 5, col: 5 },
    GlitchPatchCell { row: 5, col: 7 },
];

const GLITCH_S6_PATCH_CELLS: &[GlitchPatchCell] = &[
    GlitchPatchCell { row: 4, col: 3 },
    GlitchPatchCell { row: 4, col: 4 },
    GlitchPatchCell { row: 4, col: 5 },
    GlitchPatchCell { row: 4, col: 6 },
    GlitchPatchCell { row: 4, col: 7 },
    GlitchPatchCell { row: 5, col: 3 },
    GlitchPatchCell { row: 5, col: 4 },
    GlitchPatchCell { row: 5, col: 5 },
    GlitchPatchCell { row: 5, col: 6 },
    GlitchPatchCell { row: 5, col: 7 },
];

fn glitch_patch_allowlist(stage: Stage) -> &'static [GlitchPatchCell] {
    match stage {
        Stage::S3 => GLITCH_S3_PATCH_CELLS,
        Stage::S4 => GLITCH_S4_PATCH_CELLS,
        Stage::S5 => GLITCH_S5_PATCH_CELLS,
        Stage::S6 => GLITCH_S6_PATCH_CELLS,
        Stage::S0 | Stage::S1 | Stage::S2 => &[],
    }
}

fn span_role_at(spans: &[StyledSegment], row: usize, col: usize) -> Option<PaletteRoleName> {
    spans
        .iter()
        .find(|span| span.line == row && col >= span.start && col < span.end)
        .map(|span| span.role)
}

pub fn is_protected_glitch_face_cell(
    stage: Stage,
    row: usize,
    col: usize,
    spans: &[StyledSegment],
) -> bool {
    if matches!(
        span_role_at(spans, row, col),
        Some(PaletteRoleName::Eye | PaletteRoleName::Mouth)
    ) {
        return true;
    }

    match stage {
        Stage::S5 | Stage::S6 => {
            row == 1 || ((2..=3).contains(&row) && (3..=7).contains(&col))
        }
        _ => false,
    }
}

pub fn safe_glitch_patch_candidates(
    stage: Stage,
    lines: &[String],
    spans: &[StyledSegment],
) -> Vec<GlitchPatchCell> {
    glitch_patch_allowlist(stage)
        .iter()
        .copied()
        .filter(|cell| {
            let Some(line) = lines.get(cell.row) else {
                return false;
            };
            let Some(ch) = line.chars().nth(cell.col) else {
                return false;
            };
            ch != ' '
                && unicode_width::UnicodeWidthChar::width(ch) == Some(1)
                && !is_protected_glitch_face_cell(stage, cell.row, cell.col, spans)
        })
        .collect()
}
```

- [ ] **Step 4: Add deterministic ordering helpers**

Add:

```rust
fn hash_pet_seed(seed: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in seed.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn stage_discriminant(stage: Stage) -> u64 {
    stage.index() as u64
}

pub fn ordered_glitch_patch_cells(
    pet: &GeneratedPet,
    stage: Stage,
    day_seed: u64,
    lines: &[String],
    spans: &[StyledSegment],
) -> Vec<GlitchPatchCell> {
    if pet.species != Species::Glitch {
        return Vec::new();
    }
    let seed_hash = hash_pet_seed(&pet.seed);
    let mut scored = safe_glitch_patch_candidates(stage, lines, spans)
        .into_iter()
        .map(|cell| {
            let score = mix64(
                seed_hash
                    ^ day_seed.rotate_left(17)
                    ^ stage_discriminant(stage).rotate_left(29)
                    ^ ((cell.row as u64) << 8)
                    ^ cell.col as u64,
            );
            (score, cell)
        })
        .collect::<Vec<_>>();
    scored.sort_by_key(|(score, cell)| (*score, *cell));
    scored.into_iter().map(|(_, cell)| cell).collect()
}

pub fn selected_glitch_patch_cells(
    pet: &GeneratedPet,
    stage: Stage,
    day_seed: u64,
    tier: GlitchPatchTier,
    lines: &[String],
    spans: &[StyledSegment],
) -> Vec<GlitchPatchCell> {
    ordered_glitch_patch_cells(pet, stage, day_seed, lines, spans)
        .into_iter()
        .take(tier.max_marks())
        .collect()
}
```

- [ ] **Step 5: Run selector tests**

Run:

```bash
cargo test --lib pet::render::tests::glitch_safe_patch_candidates_exclude_face_and_outline
cargo test --lib pet::render::tests::glitch_elder_expression_island_is_protected
cargo test --lib pet::render::tests::ordered_glitch_patch_cells_are_stable_and_day_local
cargo test --lib pet::render::tests::tier_selection_reveals_prefix_without_relocating_marks
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/pet/render.rs
git commit -m "feat(pet): select safe glitch repair cells"
```

---

### Task 3: Render Persistent Repair Marks Without Overlapping Spans

**Files:**
- Modify: `src/pet/render.rs`
- Test: `src/pet/render.rs`

**Interfaces:**
- Consumes: `selected_glitch_patch_cells` from Task 2.
- Produces: persistent repair glyph mutation and `PaletteRoleName::Pattern`/`Accent` spans.

- [ ] **Step 1: Write failing repair-render tests**

Add:

```rust
#[test]
fn glitch_repair_marks_use_soft_roles_not_corruption() {
    let pet = generate_pet("glitch-repair-soft").with_species(Species::Glitch);
    let rendered = render_pet(
        &pet,
        Stage::S4,
        Mood::Content,
        AnimationFrame {
            tick: 1,
            glitch_corruption: Some(GlitchCorruptionFrame {
                day_seed: 42,
                patch_tier: GlitchPatchTier::Heavy,
                burst_level: GlitchBurstLevel::None,
                calm_mode: false,
                feed_reaction: false,
            }),
            ..AnimationFrame::default()
        },
    );

    let repair_spans = rendered
        .spans
        .iter()
        .filter(|span| matches!(span.role, PaletteRoleName::Pattern | PaletteRoleName::Accent))
        .filter(|span| span.end == span.start + 1)
        .collect::<Vec<_>>();
    let text = rendered.lines.join("\n");
    assert!(
        repair_spans.len() >= 3,
        "heavy Glitch day should emit at least three one-cell repair spans"
    );
    assert!(
        text.contains('+') || text.contains('=') || text.contains(':') || text.contains('.'),
        "heavy Glitch day should show at least one repair glyph"
    );
    assert!(
        rendered
            .spans
            .iter()
            .all(|span| span.role != PaletteRoleName::Corruption),
        "persistent repair marks must not use the loud Corruption role"
    );
}

#[test]
fn glitch_repair_spans_are_sorted_and_non_overlapping() {
    let pet = generate_pet("glitch-repair-spans").with_species(Species::Glitch);
    let rendered = render_pet(
        &pet,
        Stage::S6,
        Mood::Content,
        AnimationFrame {
            tick: 1,
            glitch_corruption: Some(GlitchCorruptionFrame {
                day_seed: 777,
                patch_tier: GlitchPatchTier::Heavy,
                burst_level: GlitchBurstLevel::None,
                calm_mode: true,
                feed_reaction: false,
            }),
            ..AnimationFrame::default()
        },
    );

    let mut spans = rendered.spans.clone();
    let sorted = {
        let mut clone = spans.clone();
        clone.sort_by_key(|span| (span.line, span.start, span.end));
        clone
    };
    assert_eq!(spans, sorted, "rendered spans should be sorted for TUI consumers");

    spans.sort_by_key(|span| (span.line, span.start));
    for pair in spans.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        if left.line == right.line {
            assert!(
                left.end <= right.start,
                "overlapping spans on line {}: {:?} then {:?}",
                left.line,
                left,
                right
            );
        }
    }
}

#[test]
fn glitch_repair_marks_do_not_touch_elder_expression_island() {
    let pet = generate_pet("glitch-elder-repair").with_species(Species::Glitch);
    let rendered = render_pet(
        &pet,
        Stage::S6,
        Mood::Content,
        AnimationFrame {
            tick: 1,
            glitch_corruption: Some(GlitchCorruptionFrame {
                day_seed: 42,
                patch_tier: GlitchPatchTier::Heavy,
                burst_level: GlitchBurstLevel::None,
                calm_mode: true,
                feed_reaction: false,
            }),
            ..AnimationFrame::default()
        },
    );

    for span in rendered
        .spans
        .iter()
        .filter(|span| matches!(span.role, PaletteRoleName::Pattern | PaletteRoleName::Accent))
        .filter(|span| span.end == span.start + 1)
    {
        let raw_row = span.line.saturating_sub(1);
        let raw_col = span.start.saturating_sub(1);
        assert!(
            !((2..=3).contains(&raw_row) && (3..=7).contains(&raw_col)),
            "repair span touched protected elder expression island: {:?}",
            span
        );
    }
}
```

- [ ] **Step 2: Run repair tests and verify failure**

Run:

```bash
cargo test --lib pet::render::tests::glitch_repair_marks_use_soft_roles_not_corruption
cargo test --lib pet::render::tests::glitch_repair_spans_are_sorted_and_non_overlapping
cargo test --lib pet::render::tests::glitch_repair_marks_do_not_touch_elder_expression_island
```

Expected: FAIL because repair rendering has not been applied.

- [ ] **Step 3: Add repair glyph and retag helpers**

Add:

```rust
const GLITCH_REPAIR_GLYPHS: &[char] = &['+', '=', ':', '.'];

fn repair_glyph_for(cell: GlitchPatchCell, day_seed: u64) -> char {
    let index = mix64(day_seed ^ ((cell.row as u64) << 8) ^ cell.col as u64) as usize
        % GLITCH_REPAIR_GLYPHS.len();
    GLITCH_REPAIR_GLYPHS[index]
}

fn repair_role_for(index: usize) -> PaletteRoleName {
    if index == 0 {
        PaletteRoleName::Accent
    } else {
        PaletteRoleName::Pattern
    }
}

fn retag_cell_as_role(
    spans: &mut Vec<StyledSegment>,
    row: usize,
    col: usize,
    role: PaletteRoleName,
) {
    let mut split: Vec<StyledSegment> = Vec::new();
    for span in spans.iter_mut() {
        if span.line != row || col < span.start || col >= span.end {
            continue;
        }
        let original_end = span.end;
        let original_role = span.role;
        span.end = col;
        if col + 1 < original_end {
            split.push(StyledSegment {
                line: row,
                start: col + 1,
                end: original_end,
                role: original_role,
            });
        }
        break;
    }
    split.push(StyledSegment { line: row, start: col, end: col + 1, role });
    spans.retain(|span| span.start < span.end);
    spans.extend(split);
    spans.sort_by_key(|span| (span.line, span.start, span.end));
}

fn retag_cell_as_corruption(spans: &mut Vec<StyledSegment>, row: usize, col: usize) {
    retag_cell_as_role(spans, row, col, PaletteRoleName::Corruption);
}

fn apply_glitch_repair_marks(
    pet: &GeneratedPet,
    stage: Stage,
    lines: &mut [String],
    spans: &mut Vec<StyledSegment>,
    frame: GlitchCorruptionFrame,
) {
    let cells = selected_glitch_patch_cells(
        pet,
        stage,
        frame.day_seed,
        frame.patch_tier,
        lines,
        spans,
    );
    for (index, cell) in cells.into_iter().enumerate() {
        let glyph = repair_glyph_for(cell, frame.day_seed);
        replace_char_in_line(&mut lines[cell.row], cell.col, glyph);
        retag_cell_as_role(spans, cell.row, cell.col, repair_role_for(index));
    }
}
```

If `retag_cell_as_corruption` already exists, replace its body with the wrapper above and keep the existing tests.

- [ ] **Step 4: Call repair marks from `render_pet`**

Update the Glitch section in `render_pet`:

```rust
    if pet.species == Species::Glitch {
        if let Some(glitch) = frame.glitch_corruption {
            apply_glitch_repair_marks(pet, stage, &mut lines, &mut spans, glitch);
            if !glitch.calm_mode {
                apply_glitch_corruption(&mut lines, &mut spans, frame.tick);
            }
        } else {
            apply_glitch_corruption(&mut lines, &mut spans, frame.tick);
        }
    }
```

- [ ] **Step 5: Run focused render tests**

Run:

```bash
cargo test --lib pet::render::tests::glitch_repair_marks_use_soft_roles_not_corruption
cargo test --lib pet::render::tests::glitch_repair_spans_are_sorted_and_non_overlapping
cargo test --lib pet::render::tests::glitch_repair_marks_do_not_touch_elder_expression_island
cargo test --lib pet::render::tests::glitch_corruption_emits_corruption_role_spans_on_active_tick
cargo test --lib pet::render::tests::glitch_corruption_never_recolors_the_eye_center
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/pet/render.rs
git commit -m "feat(pet): render glitch repair marks"
```

---

### Task 4: Wire Glitch Corruption Through Watch Rerendering

**Files:**
- Modify: `src/commands/watch.rs`
- Test: `src/commands/watch.rs` or existing command/watch tests if present

**Interfaces:**
- Consumes: `glitch_corruption_frame_for_inputs` and `GlitchCorruptionFrame` from Task 1.
- Produces: watch and round view models whose `pet_art` includes Glitch day-local repairs when the species is Glitch.

- [ ] **Step 1: Write failing watch integration tests**

Add tests to the existing `#[cfg(test)]` module in `src/commands/watch.rs`; if no module exists, add one at the bottom:

```rust
#[cfg(test)]
mod glitch_corruption_tests {
    use super::*;
    use crate::game::evolution::Stage;
    use crate::pet::generation::Species;
    use crate::pet::render::PaletteRoleName;
    use crate::storage::{
        state::PetState,
        usage_store::{NormalizedUsageEvent, UsageStore},
    };
    use tempfile::tempdir;
    use time::OffsetDateTime;

    #[test]
    fn glitch_watch_view_model_rerender_adds_day_local_repair_spans() {
        let dir = tempdir().unwrap();
        let usage_db = dir.path().join("usage.sqlite");
        let mut state = PetState::new_for_test("glitch-watch-patches", "Mux");
        state.pet.generated_species = Species::Glitch;
        state.stage = Stage::S6;
        state.calibration.daily_effective_tokens = 10_000.0;
        let now = OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap();
        let mut store = UsageStore::open(&usage_db).unwrap();
        let mut event = NormalizedUsageEvent::for_test_at(now, 18_000.0);
        event.provider_surface = "codex".to_string();
        store.insert_event(&event).unwrap();

        let vm = build_watch_view_model_for_test_at(&state, &usage_db, now).unwrap();

        assert!(
            vm.pet_spans.iter().any(|span| {
                matches!(span.role, PaletteRoleName::Pattern | PaletteRoleName::Accent)
                    && span.end == span.start + 1
            }),
            "Glitch watch VM should include soft one-cell repair spans"
        );
        assert!(
            vm.pet_spans.iter().all(|span| span.role != PaletteRoleName::Corruption),
            "day-local repair marks should not use Corruption in the steady VM"
        );
    }

    #[test]
    fn non_glitch_watch_view_model_does_not_receive_glitch_repair_spans() {
        let dir = tempdir().unwrap();
        let usage_db = dir.path().join("usage.sqlite");
        let mut state = PetState::new_for_test("fuzz-watch-patches", "Mochi");
        state.pet.generated_species = Species::Fuzz;
        state.stage = Stage::S6;
        let now = OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap();
        let _store = UsageStore::open(&usage_db).unwrap();

        let vm = build_watch_view_model_for_test_at(&state, &usage_db, now).unwrap();

        assert_eq!(vm.pet_render.generated_species, Species::Fuzz);
        assert!(
            vm.pet_spans
                .iter()
                .all(|span| span.role != PaletteRoleName::Corruption),
            "non-Glitch steady VM should not get Glitch corruption roles"
        );
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test --lib commands::watch::glitch_corruption_tests::glitch_watch_view_model_rerender_adds_day_local_repair_spans
cargo test --lib commands::watch::glitch_corruption_tests::non_glitch_watch_view_model_does_not_receive_glitch_repair_spans
```

Expected: FAIL because the watch path does not pass `glitch_corruption`.

- [ ] **Step 3: Add a watch adapter helper**

Add near `rerender_pet_for_view_model`:

```rust
fn glitch_corruption_frame_for_view_model(
    vm: &WatchViewModel,
    now: time::OffsetDateTime,
) -> Option<crate::pet::render::GlitchCorruptionFrame> {
    if vm.pet_render.generated_species != Species::Glitch {
        return None;
    }
    let feed_reaction = crate::pet::animator::compute_token_pop(vm.last_feed_pulse_at, now).is_some();
    Some(crate::pet::render::glitch_corruption_frame_for_inputs(
        vm.day_context.date_seed,
        vm.day_context.today_ratio,
        vm.life_profile.burst_level,
        vm.life_profile.calm_mode,
        feed_reaction,
    ))
}
```

- [ ] **Step 4: Pass the helper into `rerender_pet_for_view_model`**

Modify the `AnimationFrame` literal inside `rerender_pet_for_view_model`:

```rust
        AnimationFrame {
            tick,
            blink_suppression_ticks: 0,
            hold_eyes_closed,
            blink_slowdown: crate::pet::render::blink_slowdown_for_tiredness(
                vm.day_context.tiredness,
            ),
            soft_eyes: matches!(
                pet_performance,
                crate::tui::room::PetPerformance::TiredAwake
                    | crate::tui::room::PetPerformance::HeavyDayCozy
            ),
            work_accent: work_accent_for_profile(&vm.life_profile),
            feed_reaction: crate::pet::animator::compute_token_pop(vm.last_feed_pulse_at, now)
                .is_some(),
            glitch_corruption: glitch_corruption_frame_for_view_model(vm, now),
        },
```

- [ ] **Step 5: Rerender before returning from `build_watch_view_model_at`**

Change the final `Ok(WatchViewModel { ... })` into:

```rust
    let mut vm = WatchViewModel {
        pet_art: rendered.lines,
        pet_spans: rendered.spans,
        pet_render: PetRenderModel {
            seed: state.pet.seed.clone(),
            generated_species: state.pet.generated_species,
            stage: state.stage,
            mood,
        },
        pet_palette,
        habitat: build_habitat_view(state),
        life_profile,
        activity_identity,
        day_context,
        pet_name: state.pet.accepted_name.clone(),
        species: species.as_str().to_string(),
        stage: stage_label(species, stage).to_string(),
        mood: mood.as_str().to_string(),
        age_days: (now - state.created_at).whole_days().max(0) as u32,
        fed: state.vitals.fed / 100.0,
        happiness: state.vitals.happiness / 100.0,
        energy: state.vitals.energy / 100.0,
        today_effective_tokens: today_total_tokens,
        recent_daily_effective_tokens: usage_store
            .seven_day_token_history(now, mapper)
            .unwrap_or_else(|_| vec![0.0; 7]),
        source_breakdown,
        source_health,
        current_bucket_effective_tokens: last_10m_total_tokens,
        recent_events,
        helper_status,
        errors,
        latest_evolution: state
            .seen_stage_transitions
            .last()
            .map(|stage| stage.as_str().to_string()),
        cursor_screen: None,
        mouse_tracking_enabled: true,
        current_speech: crate::pet::speech::current_pet_speech_for_scene(
            mood,
            &crate::tui::life::PetLifeProfile::default(),
            &day_context,
            now,
        ),
        wander_offset_x: 0,
        breath_offset_y: crate::pet::animator::compute_breath_offset_with_rhythm(
            Some(species),
            now,
            crate::pet::animator::breath_rhythm_for_day(&day_context),
        ),
        facing: 1,
        last_feed_pulse_at: None,
        progress: {
            let rate_per_hour: f64 = usage_store
                .canonical_total_tokens_between(now - Duration::hours(1), now)
                .unwrap_or(0.0);
            let is_max = matches!(stage, Stage::S6);
            let stage_start = stage_start_xp(stage);
            let xp_in_stage = state.xp - stage_start;
            let xp_to_next = next_stage_xp_target(stage) - stage_start;
            let fraction = if xp_to_next <= 0.0 || is_max {
                1.0
            } else {
                (xp_in_stage / xp_to_next).clamp(0.0, 1.0) as f32
            };
            let next_stage_label = if is_max {
                "—".to_string()
            } else {
                let next = match stage {
                    Stage::S0 => Stage::S1,
                    Stage::S1 => Stage::S2,
                    Stage::S2 => Stage::S3,
                    Stage::S3 => Stage::S4,
                    Stage::S4 => Stage::S5,
                    Stage::S5 => Stage::S6,
                    Stage::S6 => Stage::S6,
                };
                stage_label(species, next).to_string()
            };
            ProgressView {
                stage_label: stage_label(species, stage).to_string(),
                next_stage_label,
                fraction,
                xp_in_stage,
                xp_to_next,
                rate_per_hour,
                is_max_stage: is_max,
            }
        },
        bio: {
            let age = now - state.created_at;
            let age_label = BioView::format_age(age);
            let local = state.created_at.to_offset(local_offset);
            let month_name = match local.month() {
                time::Month::January => "jan",
                time::Month::February => "feb",
                time::Month::March => "mar",
                time::Month::April => "apr",
                time::Month::May => "may",
                time::Month::June => "jun",
                time::Month::July => "jul",
                time::Month::August => "aug",
                time::Month::September => "sep",
                time::Month::October => "oct",
                time::Month::November => "nov",
                time::Month::December => "dec",
            };
            let hatched_label = format!(
                "{} {:02} {:02}:{:02}",
                month_name,
                local.day(),
                local.hour(),
                local.minute(),
            );
            BioView { hatched_label, age_label }
        },
    };
    rerender_pet_for_view_model(
        &mut vm,
        now.unix_timestamp().max(0) as u64,
        day_context.asleep,
        now,
    )?;
    Ok(vm)
```

- [ ] **Step 6: Run watch integration tests**

Run:

```bash
cargo test --lib commands::watch::glitch_corruption_tests::glitch_watch_view_model_rerender_adds_day_local_repair_spans
cargo test --lib commands::watch::glitch_corruption_tests::non_glitch_watch_view_model_does_not_receive_glitch_repair_spans
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/commands/watch.rs
git commit -m "feat(watch): rerender glitch repair state from day context"
```

---

### Task 5: Add Preview Lab Contract Frames

**Files:**
- Modify: `tests/dev_preview.rs`
- Modify: `src/dev_preview/pets.rs`
- Modify: `src/dev_preview/watch.rs`
- Modify: `src/dev_preview/round.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Test: `tests/dev_preview.rs`

**Interfaces:**
- Consumes: watch rerendering from Task 4.
- Produces: `pet-glitch-persistence-states`, `watch-glitch-patched-quiet`, `watch-glitch-patched-active`, `watch-glitch-burst`, `watch-glitch-calm-hot`, and `round-glitch-patched-s6` preview frames.

- [ ] **Step 1: Write failing Preview Lab tests**

Add constants near the existing preview id constants in `tests/dev_preview.rs`:

```rust
const GLITCH_PERSISTENCE_PET_ID: &str = "pet-glitch-persistence-states";

const GLITCH_PERSISTENCE_WATCH_IDS: [&str; 4] = [
    "watch-glitch-patched-quiet",
    "watch-glitch-patched-active",
    "watch-glitch-burst",
    "watch-glitch-calm-hot",
];

const GLITCH_PERSISTENCE_ROUND_ID: &str = "round-glitch-patched-s6";
```

Add tests:

```rust
#[test]
fn dev_preview_glitch_persistence_pet_frame_records_patch_contract() {
    let run = PreviewRun::new();

    run.run_success("pets");

    assert!(
        run.out
            .join(format!("frames/{GLITCH_PERSISTENCE_PET_ID}.txt"))
            .is_file()
    );
    let manifest = run.manifest();
    let scenario = scenario(&manifest, GLITCH_PERSISTENCE_PET_ID);
    assert_eq!(scenario["kind"], "pet-matrix");
    assert_eq!(scenario["inputs"]["species"], "glitch");
    assert!(scenario["inputs"]["date_seed"].as_u64().unwrap() > 0);
    assert_eq!(scenario["inputs"]["same_day_restart"], true);
    assert_eq!(scenario["inputs"]["next_dawn_reset"], true);
    assert!(scenario["inputs"]["selected_patch_cells"].as_array().unwrap().len() >= 3);
    assert!(scenario["inputs"]["protected_face_cells"].as_array().unwrap().len() >= 6);
}

#[test]
fn dev_preview_glitch_watch_frames_record_patch_inputs() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let manifest = run.manifest();
    for id in GLITCH_PERSISTENCE_WATCH_IDS {
        assert!(run.out.join(format!("frames/{id}.txt")).is_file(), "missing {id}.txt");
        assert!(
            run.out.join(format!("frames/{id}.cells.json")).is_file(),
            "missing {id}.cells.json"
        );
        assert!(
            run.out.join(format!("frames/{id}.layout.json")).is_file(),
            "missing {id}.layout.json"
        );
        let scenario = scenario(&manifest, id);
        assert_eq!(scenario["kind"], "watch");
        assert_eq!(scenario["inputs"]["species"], "glitch");
        assert!(scenario["inputs"]["date_seed"].as_u64().unwrap() > 0);
        assert!(scenario["inputs"]["patch_tier"].is_string());
        assert!(scenario["inputs"]["burst_level"].is_string());
        assert!(scenario["inputs"]["expected_patch_count"].as_u64().unwrap() <= 3);
        assert!(scenario["inputs"]["selected_patch_cells"].is_array());
        assert!(scenario["inputs"]["protected_face_cells"].is_array());
    }
}

#[test]
fn dev_preview_round_glitch_patched_s6_records_patch_contract() {
    let run = PreviewRun::new();

    run.run_success("round");

    assert!(
        run.out
            .join(format!("frames/{GLITCH_PERSISTENCE_ROUND_ID}.txt"))
            .is_file()
    );
    let manifest = run.manifest();
    let scenario = scenario(&manifest, GLITCH_PERSISTENCE_ROUND_ID);
    assert_eq!(scenario["kind"], "round");
    assert_eq!(scenario["inputs"]["species"], "glitch");
    assert_eq!(scenario["inputs"]["stage"], "s6");
    assert!(scenario["inputs"]["selected_patch_cells"].as_array().unwrap().len() >= 1);
    assert_eq!(scenario["round"]["aperture"]["shape"], "circle");
}
```

- [ ] **Step 2: Run Preview Lab tests and verify failure**

Run:

```bash
cargo test --features dev-preview --test dev_preview dev_preview_glitch_persistence_pet_frame_records_patch_contract
cargo test --features dev-preview --test dev_preview dev_preview_glitch_watch_frames_record_patch_inputs
cargo test --features dev-preview --test dev_preview dev_preview_round_glitch_patched_s6_records_patch_contract
```

Expected: FAIL because the new frames do not exist.

- [ ] **Step 3: Add pet persistence frame**

In `src/dev_preview/pets.rs`, add `render_glitch_persistence_states(ctx)` to `pet_frames` after `render_glitch_live_states(ctx)`.

Use this fixture shape:

```rust
#[derive(Clone, Copy)]
struct GlitchPersistenceFixture {
    label: &'static str,
    stage: Stage,
    date_seed: u64,
    patch_tier: GlitchPatchTier,
    burst_level: GlitchBurstLevel,
    calm_mode: bool,
    feed_reaction: bool,
    tick: u64,
}
```

Render the fixture with:

```rust
let rendered = render_pet(
    &pet,
    fixture.stage,
    Mood::Content,
    AnimationFrame {
        tick: fixture.tick,
        glitch_corruption: Some(GlitchCorruptionFrame {
            day_seed: fixture.date_seed,
            patch_tier: fixture.patch_tier,
            burst_level: fixture.burst_level,
            calm_mode: fixture.calm_mode,
            feed_reaction: fixture.feed_reaction,
        }),
        ..AnimationFrame::default()
    },
);
```

After `frame_from_buffer`, set `frame.extra_inputs`:

```rust
let selected_patch_cells = selected_glitch_patch_cells(
    &pet,
    Stage::S6,
    42,
    GlitchPatchTier::Heavy,
    &raw_lines,
    &raw_spans,
);
let selected_patch_cells_json = selected_patch_cells
    .iter()
    .map(|cell| json!({"row": cell.row, "col": cell.col}))
    .collect::<Vec<_>>();

frame.extra_inputs = BTreeMap::from([
    ("species".to_string(), json!("glitch")),
    ("fixture".to_string(), json!("glitch-persistence-states")),
    ("date_seed".to_string(), json!(42_u64)),
    ("patch_tier".to_string(), json!("heavy")),
    ("burst_level".to_string(), json!("strong")),
    ("calm_mode".to_string(), json!(false)),
    ("feed_reaction".to_string(), json!(true)),
    ("expected_patch_count".to_string(), json!(3)),
    ("selected_patch_cells".to_string(), json!(selected_patch_cells_json)),
    ("protected_face_cells".to_string(), json!([
        {"row": 1, "col": 4},
        {"row": 1, "col": 5},
        {"row": 1, "col": 6},
        {"row": 2, "col": 3},
        {"row": 2, "col": 4},
        {"row": 2, "col": 5},
        {"row": 2, "col": 6},
        {"row": 2, "col": 7},
        {"row": 3, "col": 3},
        {"row": 3, "col": 4},
        {"row": 3, "col": 5},
        {"row": 3, "col": 6},
        {"row": 3, "col": 7}
    ])),
    ("same_day_restart".to_string(), json!(true)),
    ("next_dawn_reset".to_string(), json!(true)),
]);
```

The `raw_lines` and `raw_spans` values should come from the same raw Glitch S6 render used to draw the frame, before the 13x10 pet frame offset is added.

- [ ] **Step 4: Add watch Glitch fixtures**

In `src/dev_preview/watch.rs`, add a `glitch_pet_state(ctx)` helper:

```rust
fn glitch_pet_state(ctx: &PreviewRenderContext) -> PetState {
    let mut state = seeded_pet_state(ctx);
    state.pet.seed = "glorp-preview-glitch-persistence".to_string();
    state.pet.accepted_name = "Mux".to_string();
    state.pet.generated_species = Species::Glitch;
    state.stage = Stage::S6;
    state.xp = 72.0;
    state.lifetime_effective_tokens = 2_400_000.0;
    state.vitals = Vitals { fed: 86.0, happiness: 88.0, energy: 84.0 };
    state
}
```

Add `glitch_persistence_frame_fixtures(ctx)` returning four `DayContextFrameFixture` values:

```rust
fn glitch_persistence_frame_fixtures(ctx: &PreviewRenderContext) -> Vec<DayContextFrameFixture> {
    let now = ctx.fixed_now + Duration::hours(4);
    vec![
        DayContextFrameFixture {
            id: "watch-glitch-patched-quiet",
            title: "Watch Glitch Patched Quiet",
            width: 120,
            height: 32,
            now,
            state: glitch_pet_state,
            life: WatchLifeFixture {
                profile: idle_life_profile(),
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: preview_glitch_day_context(now, 0.4, 42),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-glitch-patched-active",
            title: "Watch Glitch Patched Active",
            width: 120,
            height: 32,
            now,
            state: glitch_pet_state,
            life: WatchLifeFixture {
                profile: warm_life_profile(false),
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: None,
            },
            day_context: preview_glitch_day_context(now, 1.0, 42),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-glitch-burst",
            title: "Watch Glitch Burst",
            width: 120,
            height: 32,
            now,
            state: glitch_pet_state,
            life: WatchLifeFixture {
                profile: hot_life_profile(false),
                color_capability: ColorCapability::Truecolor,
                last_feed_pulse_at: Some(now - Duration::milliseconds(400)),
            },
            day_context: preview_glitch_day_context(now, 1.7, 42),
            hold_eyes_closed: false,
        },
        DayContextFrameFixture {
            id: "watch-glitch-calm-hot",
            title: "Watch Glitch Calm Hot",
            width: 120,
            height: 32,
            now,
            state: glitch_pet_state,
            life: WatchLifeFixture {
                profile: hot_life_profile(true),
                color_capability: ColorCapability::Flat,
                last_feed_pulse_at: Some(now - Duration::milliseconds(400)),
            },
            day_context: preview_glitch_day_context(now, 1.7, 42),
            hold_eyes_closed: false,
        },
    ]
}
```

Make `preview_glitch_day_context` start from an existing day-context fixture and override only deterministic fields:

```rust
fn preview_glitch_day_context(now: OffsetDateTime, today_ratio: f32, date_seed: u64) -> DayContext {
    let mut day = heavy_day_evening_day_context(now);
    day.today_ratio = today_ratio;
    day.date_seed = date_seed;
    day.asleep = false;
    day
}
```

Push these fixtures from `watch_frames` by calling `render_day_context_watch_frame`.

- [ ] **Step 5: Add round Glitch patched frame**

In `src/dev_preview/round.rs`, add:

```rust
let mut patched_glitch = WatchViewModel::fixture_with_habitat_props();
patched_glitch.pet_render.seed = "glorp-preview-glitch-persistence".to_string();
patched_glitch.pet_render.generated_species = crate::pet::generation::Species::Glitch;
patched_glitch.pet_render.stage = crate::game::evolution::Stage::S6;
patched_glitch.day_context.date_seed = 42;
patched_glitch.day_context.today_ratio = 1.7;
patched_glitch.life_profile.burst_level = 0.0;
patched_glitch.life_profile.calm_mode = true;
crate::commands::watch::rerender_pet_for_view_model(
    &mut patched_glitch,
    ctx.fixed_now.unix_timestamp().max(0) as u64,
    false,
    ctx.fixed_now,
)
.expect("round preview fixture should rerender");
frames.push(frame(
    "round-glitch-patched-s6",
    "Round Glitch Patched S6",
    &patched_glitch,
    ctx,
    RoundRenderCapabilities::preview_truecolor(),
));
```

- [ ] **Step 6: Add manifest input branches**

In `src/dev_preview/scenarios.rs`, update `scenario_metadata`:

```rust
"pet-glitch-persistence-states" => (
    PreviewScenarioKind::PetMatrix,
    "Review deterministic Glitch day-local repair memory across quiet, active, burst, same-day restart, and next-dawn reset states.",
    frame.extra_inputs.clone(),
    vec![
        "Confirm repaired marks read as cute self-repair rather than injury.".to_string(),
        "Check same-day restart preserves marks and next-dawn reset moves them.".to_string(),
        "Verify S5/S6 keep a living elder expression.".to_string(),
    ],
),
id if id.starts_with("watch-glitch-") => (
    PreviewScenarioKind::Watch,
    "Review Glitch persistent repair marks and transient glitch states in the full watch layout.",
    frame.extra_inputs.clone(),
    vec![
        "Confirm patch marks remain legible in the pet scene.".to_string(),
        "Check calm mode suppresses loud corruption while keeping calm repair marks.".to_string(),
        "Verify no provider or source identity appears in Glitch behavior inputs.".to_string(),
    ],
),
```

For `round-glitch-patched-s6`, replace `round_inputs(ctx)` with
`round_inputs_for_frame(&frame, ctx)` and add the patched Glitch branch there:

```rust
fn round_inputs_for_frame(frame: &PreviewFrame, ctx: &PreviewRenderContext) -> BTreeMap<String, Value> {
    let mut inputs = round_inputs(ctx);
    if frame.id == "round-glitch-patched-s6" {
        inputs.extend([
            ("species".to_string(), json!("glitch")),
            ("stage".to_string(), json!("s6")),
            ("date_seed".to_string(), json!(42_u64)),
            ("patch_tier".to_string(), json!("heavy")),
            ("burst_level".to_string(), json!("none")),
            ("calm_mode".to_string(), json!(true)),
            ("feed_reaction".to_string(), json!(false)),
            ("expected_patch_count".to_string(), json!(3)),
            ("selected_patch_cells".to_string(), json!([
                {"row": 4, "col": 5}
            ])),
            ("protected_face_cells".to_string(), json!([
                {"row": 1, "col": 4},
                {"row": 1, "col": 5},
                {"row": 1, "col": 6},
                {"row": 2, "col": 3},
                {"row": 2, "col": 4},
                {"row": 2, "col": 5},
                {"row": 2, "col": 6},
                {"row": 2, "col": 7},
                {"row": 3, "col": 3},
                {"row": 3, "col": 4},
                {"row": 3, "col": 5},
                {"row": 3, "col": 6},
                {"row": 3, "col": 7}
            ])),
        ]);
    }
    inputs
}
```

Then update `round_bundle`:

```rust
let inputs = round_inputs_for_frame(&frame, ctx);
PreviewScenarioBundle::from_parts(
    frame,
    PreviewScenarioKind::Round,
    "Review round macOS companion preview with aperture masking and privacy metadata.",
    inputs,
    Some(round),
    vec![
        "Confirm the circular aperture masks the frame corners.".to_string(),
        "Check that dashboard labels and source diagnostics are not visible.".to_string(),
        "Verify privacy metadata records all visibility flags as false.".to_string(),
    ],
)
```

- [ ] **Step 7: Run Preview Lab tests**

Run:

```bash
cargo test --features dev-preview --test dev_preview dev_preview_glitch_persistence_pet_frame_records_patch_contract
cargo test --features dev-preview --test dev_preview dev_preview_glitch_watch_frames_record_patch_inputs
cargo test --features dev-preview --test dev_preview dev_preview_round_glitch_patched_s6_records_patch_contract
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add tests/dev_preview.rs src/dev_preview/pets.rs src/dev_preview/watch.rs src/dev_preview/round.rs src/dev_preview/scenarios.rs
git commit -m "feat(dev-preview): add glitch persistence fixtures"
```

---

### Task 6: Add Session Burst Behavior and Calm Suppression

**Files:**
- Modify: `src/pet/render.rs`
- Test: `src/pet/render.rs`

**Interfaces:**
- Consumes: `GlitchBurstLevel` and `GlitchCorruptionFrame`.
- Produces: feed/burst transient corruption that remains separate from day-local repair marks.

- [ ] **Step 1: Write failing transient tests**

Add:

```rust
#[test]
fn glitch_feed_reaction_can_trigger_transient_corruption_off_gate() {
    let pet = generate_pet("glitch-feed-burst").with_species(Species::Glitch);
    let rendered = render_pet(
        &pet,
        Stage::S4,
        Mood::Happy,
        AnimationFrame {
            tick: 2,
            feed_reaction: true,
            glitch_corruption: Some(GlitchCorruptionFrame {
                day_seed: 42,
                patch_tier: GlitchPatchTier::Quiet,
                burst_level: GlitchBurstLevel::Small,
                calm_mode: false,
                feed_reaction: true,
            }),
            ..AnimationFrame::default()
        },
    );

    assert!(
        rendered
            .spans
            .iter()
            .any(|span| span.role == PaletteRoleName::Corruption),
        "feed reaction should allow a short transient glitch off the old gate"
    );
}

#[test]
fn glitch_calm_mode_suppresses_transient_corruption_but_keeps_repairs() {
    let pet = generate_pet("glitch-calm-burst").with_species(Species::Glitch);
    let rendered = render_pet(
        &pet,
        Stage::S4,
        Mood::Content,
        AnimationFrame {
            tick: 13,
            feed_reaction: true,
            glitch_corruption: Some(GlitchCorruptionFrame {
                day_seed: 42,
                patch_tier: GlitchPatchTier::Heavy,
                burst_level: GlitchBurstLevel::Strong,
                calm_mode: true,
                feed_reaction: true,
            }),
            ..AnimationFrame::default()
        },
    );

    assert!(
        rendered
            .spans
            .iter()
            .all(|span| span.role != PaletteRoleName::Corruption),
        "calm mode should suppress transient corruption"
    );
    assert!(
        rendered
            .spans
            .iter()
            .any(|span| matches!(span.role, PaletteRoleName::Pattern | PaletteRoleName::Accent)),
        "calm mode should keep day-local repair marks"
    );
}
```

- [ ] **Step 2: Run transient tests and verify failure**

Run:

```bash
cargo test --lib pet::render::tests::glitch_feed_reaction_can_trigger_transient_corruption_off_gate
cargo test --lib pet::render::tests::glitch_calm_mode_suppresses_transient_corruption_but_keeps_repairs
```

Expected: FAIL because off-gate feed reaction does not yet force transient corruption.

- [ ] **Step 3: Add transient selector wrapper**

Refactor the current corruption path like this:

```rust
fn corruption_cells_for_moment(
    art_lines: &[String],
    tick: u64,
    burst_level: GlitchBurstLevel,
    feed_reaction: bool,
) -> Vec<(usize, usize)> {
    let force = feed_reaction || matches!(burst_level, GlitchBurstLevel::Strong);
    if !force {
        return corruption_cells_for_tick(art_lines, tick);
    }

    let mut candidates = Vec::new();
    for (row, line) in art_lines.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if ch != ' ' {
                candidates.push((row, col));
            }
        }
    }
    if candidates.is_empty() {
        return Vec::new();
    }
    let count = match burst_level {
        GlitchBurstLevel::None => 1,
        GlitchBurstLevel::Small => 2,
        GlitchBurstLevel::Strong => CORRUPTION_MAX_CELLS,
    }
    .min(candidates.len());
    let start = (tick.wrapping_mul(5) as usize) % candidates.len();
    candidates.rotate_left(start);
    candidates.truncate(count);
    candidates.sort_unstable();
    candidates
}

fn apply_glitch_transient_corruption(
    lines: &mut [String],
    spans: &mut Vec<StyledSegment>,
    stage: Stage,
    tick: u64,
    burst_level: GlitchBurstLevel,
    feed_reaction: bool,
) {
    let cells = corruption_cells_for_moment(lines, tick, burst_level, feed_reaction);
    if cells.is_empty() {
        return;
    }
    for (i, (row, col)) in cells.into_iter().enumerate() {
        if is_protected_glitch_face_cell(stage, row, col, spans)
            || is_eye_center(spans, row, col)
        {
            continue;
        }
        let noise =
            GLITCH_NOISE[((tick as usize).wrapping_mul(3).wrapping_add(i)) % GLITCH_NOISE.len()];
        replace_char_in_line(&mut lines[row], col, noise);
        retag_cell_as_corruption(spans, row, col);
    }
}
```

The function receives `stage: Stage` and passes that value into
`is_protected_glitch_face_cell`.

- [ ] **Step 4: Update `render_pet` to use the transient wrapper**

Use:

```rust
    if pet.species == Species::Glitch {
        if let Some(glitch) = frame.glitch_corruption {
            apply_glitch_repair_marks(pet, stage, &mut lines, &mut spans, glitch);
            if !glitch.calm_mode {
                apply_glitch_transient_corruption(
                    &mut lines,
                    &mut spans,
                    stage,
                    frame.tick,
                    glitch.burst_level,
                    glitch.feed_reaction || frame.feed_reaction,
                );
            }
        } else {
            apply_glitch_corruption(&mut lines, &mut spans, frame.tick);
        }
    }
```

- [ ] **Step 5: Run transient and existing corruption tests**

Run:

```bash
cargo test --lib pet::render::tests::glitch_feed_reaction_can_trigger_transient_corruption_off_gate
cargo test --lib pet::render::tests::glitch_calm_mode_suppresses_transient_corruption_but_keeps_repairs
cargo test --lib pet::render::tests::glitch_corruption_emits_corruption_role_spans_on_active_tick
cargo test --lib pet::render::tests::glitch_corruption_quiet_off_gate_tick
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/pet/render.rs
git commit -m "feat(pet): add glitch transient repair bursts"
```

---

### Task 7: Final Preview Review and Regression Gate

**Files:**
- Modify only files needed to fix issues found by the commands below.
- Test: full focused Glitch/Preview Lab command set.

**Interfaces:**
- Consumes all previous tasks.
- Produces the go/no-go artifact bundle for visual review.

- [ ] **Step 1: Run focused unit tests**

Run:

```bash
cargo test --lib pet::render::tests::glitch_patch_tier_quantizes_today_ratio_without_live_activity
cargo test --lib pet::render::tests::glitch_safe_patch_candidates_exclude_face_and_outline
cargo test --lib pet::render::tests::glitch_repair_marks_use_soft_roles_not_corruption
cargo test --lib pet::render::tests::glitch_feed_reaction_can_trigger_transient_corruption_off_gate
cargo test --lib pet::render::tests::glitch_corruption_never_recolors_the_eye_center
```

Expected: each command exits 0.

- [ ] **Step 2: Run Preview Lab tests**

Run:

```bash
cargo test --features dev-preview --test dev_preview dev_preview_glitch_persistence_pet_frame_records_patch_contract
cargo test --features dev-preview --test dev_preview dev_preview_glitch_watch_frames_record_patch_inputs
cargo test --features dev-preview --test dev_preview dev_preview_round_glitch_patched_s6_records_patch_contract
cargo test --features dev-preview --test dev_preview dev_preview_pets_writes_species_stage_matrix
cargo test --test round_scene
```

Expected: each command exits 0.

- [ ] **Step 3: Generate review bundle**

Run:

```bash
cargo run -- dev-preview --scenario pets --out target/glorp-preview
cargo run -- dev-preview --scenario watch --out target/glorp-preview
cargo run -- dev-preview --scenario round --out target/glorp-preview
```

Expected: each command exits 0 and writes `target/glorp-preview/index.html`.

- [ ] **Step 4: Inspect the generated manifest contract**

Run:

```bash
jq '.scenarios[] | select(.id | test("glitch-(persistence|patched|burst|calm)")) | {id, kind, inputs}' target/glorp-preview/manifest.json
```

Expected: each Glitch scenario includes `date_seed`, `patch_tier`, `burst_level`, `calm_mode`, `feed_reaction`, `expected_patch_count`, `selected_patch_cells`, and `protected_face_cells`.

- [ ] **Step 5: Visual review**

Open:

```bash
open target/glorp-preview/index.html
```

Review:

- S4/S5/S6 Glitch repaired marks read as self-repair, not damage.
- Same-day restart and next-dawn reset cells differ as intended.
- S5/S6 keep a living elder expression in truecolor and flat-color.
- `watch-glitch-calm-hot` keeps repair marks without loud corruption.
- `round-glitch-patched-s6` shows at least one declared patch mark inside the circular aperture.

- [ ] **Step 6: Commit final fixes**

If the review required tuning, stage only the touched files:

```bash
git status --short
git add src/pet/render.rs src/dev_preview/pets.rs src/dev_preview/watch.rs src/dev_preview/round.rs src/dev_preview/scenarios.rs tests/dev_preview.rs tests/round_scene.rs
git commit -m "fix(pet): tune glitch persistence previews"
```

If no tuning was needed, skip the commit.

---

## Final Verification Before Handoff

Run:

```bash
cargo fmt --check
cargo test --lib pet::render::tests::glitch_patch_tier_quantizes_today_ratio_without_live_activity
cargo test --lib pet::render::tests::glitch_safe_patch_candidates_exclude_face_and_outline
cargo test --lib pet::render::tests::glitch_repair_marks_use_soft_roles_not_corruption
cargo test --lib pet::render::tests::glitch_feed_reaction_can_trigger_transient_corruption_off_gate
cargo test --features dev-preview --test dev_preview dev_preview_glitch_persistence_pet_frame_records_patch_contract
cargo test --features dev-preview --test dev_preview dev_preview_glitch_watch_frames_record_patch_inputs
cargo test --features dev-preview --test dev_preview dev_preview_round_glitch_patched_s6_records_patch_contract
cargo test --test round_scene
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

Expected: all commands exit 0.

Use `target/glorp-preview/index.html` as the final visual review artifact.
