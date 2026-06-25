# Glorp companion — "tank in a growth ring" redesign

**Date:** 2026-06-24
**Surface:** macOS round companion (`glorp companion`), rendered fullscreen on a
2.1″ 960×960 round display.
**Builds on:** `2026-06-13-glorp-macos-round-companion-design.md` (original
companion) and `2026-06-22-glorp-pet-scene-render-seam-design.md` (the
`PetScene → SurfaceStyle → SceneDrawList` seam).

## Goal

Make the round companion feel like a living pet in a tank, glanceable on a small
round screen. Three problems with the current build drive the change:

1. The pet floats small in a starfield void — it reads as "small and lost," not
   "hero."
2. Stats are scattered and low-contrast — the three ambient vital ticks are drawn
   at 0.30 alpha (invisible), the stage labels (`shade` / `phantom-pup`) read as
   two stray words, and a horizontal bar is pinned across a round frame.
3. Props and motion are noise-tier.

The redesign keeps everything that's good — the free-float tank, the drifting
pet, the props — and levels them up, while reframing the stats to suit a round
face.

**Scope of impact (corrected):** the terminal **watch is untouched** — it renders
through a separate `PetPanel::render` path. But this work is *not* purely
companion-local: `build_round_scene_draw_list` and the drift tunables are shared
with the **live macOS menubar popover** (`src/menubar/render.rs:59`), the preview
lab (`src/round/preview.rs`), and several golden tests. The design isolates the
companion's new behavior so those surfaces keep their current feel (see Module
placement & shared-surface impact).

## Validated design

A single composition, **"free-float tank in a growth ring"**:

- **Tank** — unchanged free-float porthole (no floor). A subtle radial depth
  gradient (darker at the rim) so it reads as looking *into* something.
- **Pet** — bigger, and roaming the tank. Always fully on-screen (**bounded** —
  never clipped by the rim or overlapping the ring/stat). Pet scale and roam range
  are **device-tuned knobs** (see the size↔roam tradeoff below).
- **Growth ring** — an open-bottom arc around the rim showing stage progress.
  Felt, not measured: no percentage, no ETA. An orbiting comet on the ring tracks
  live token rate.
- **Mood aura** — a soft radial glow behind the pet, color = mood, that travels
  with the pet. Replaces the invisible vital ticks. Wellbeing is felt through the
  aura color and the pet's eyes (which already shift by mood), not charted.
- **One clean stat** — the token + rate readout, nested in the ring's bottom gap.

This satisfies the brief (bigger pet, nicer stats, bigger props) and honors the
companion-not-optimizer spirit: the ring and aura are ambient, not dashboards.

### The size↔roam tradeoff (device-tuned)

Bigger pet and wider roam directly compete. On a square 960×960 face the
monospace cell is ~2:1 (tall), so with `COMPANION_TARGET_COLS≈30` the grid is
only ~30×15 cells: `safe_y = grid_rows/2 − PET_H/2 = 7 − 5 = 2` cells of vertical
headroom. There is generous *horizontal* room (`safe_x ≈ 9`) but almost no
vertical room once the pet is large. So:

- Roam is **horizontal-dominant** by nature; meaningful 2D "swimming" requires
  giving back columns (smaller pet).
- **Decision:** ship pet-scale and roam-range as knobs with a roam-preserving
  default, and tune the exact balance on the physical screen (the 2:1 cell metric
  and font advance are device-measured, so the sweet spot is a look-at-it value).

## Components

### 1. Bigger pet + bigger props — one lever

The pet art is a fixed `PET_W=13 × PET_H=10` cells inside a `COMPANION_TARGET_COLS`
(= 36, `src/companion/app.rs:606`) grid. Lowering it derives a larger `font_size`
(`app.rs:643`); the floating props are glyph cells in the *same* `SceneDrawList`
blitted at that font size, so the pet and props grow together. **Verified
companion-local:** `COMPANION_TARGET_COLS` is referenced only in
`src/companion/app.rs`, so this size lever does not touch any other surface.

Expose pet scale as the tunable (initially via `COMPANION_TARGET_COLS`, final
value tuned on-device). Prop *count/placement* is left alone: that lives in the
shared seam (`src/tui/panels/pet/props.rs`) and would affect the watch. The
cell-size lever is expected to suffice.

### 2. Pet roams the tank (bounded, parameterized)

`companion_drift` (`src/round/scene.rs:61`) eases the pet between deterministic 2D
targets every `DRIFT_PERIOD_SECS` via smoothstep. It stays pure and golden-testable.

**Parameterize, don't mutate the shared globals.** `DRIFT_X_FRAC` / `DRIFT_Y_FRAC`
/ `DRIFT_PERIOD_SECS` are module-level consts (`scene.rs:31-41`) consumed by
`companion_drift`, which is reached from `build_round_scene_draw_list` —
**shared with the menubar popover**. To widen only the companion's roam, thread a
small motion-config (roam fractions, period, optional upward bias) into
`build_round_scene_draw_list`; the companion passes the tuned values, while the
menubar / preview / goldens pass the current defaults and keep their feel.

**Bounded geometry — the real constraints:**

- The reachable set of the drift is a **box, not an ellipse**: `nx` and `ny` come
  from independent hashes (`scene.rs:86-91`), so the pet center can reach a
  *corner* at distance `sqrt(x_radius² + y_radius²)`. Fix the misleading
  "constrained to an ellipse" doc-comment (`scene.rs:56-59`).
- The only runtime guard is a **rectangular** grid clamp (`scene.rs:106-107`); the
  AppKit aperture is a **pixel circle** that silently *cuts* anything outside it
  (`app.rs:467-477`). A corner drift outside that circle is the exact in/out
  clip we forbade.
- Therefore keep `DRIFT_X_FRAC` modest (≈0.45, **not** 0.70) and vertical roam
  gentle; bias the ellipse up only slightly (the bottom-stat reservation is the
  real competitor for the ~2 cells of vertical budget).

**Bounded-invariant test (must be sound):** sample the drift at box corners
(`fx, fy ∈ {−1, 0, +1}` jointly, not just center), map the four corners of the
`13×10` pet rect **cell→pixel** using the real `cell_w`/`cell_h` (~2:1), and assert
they sit inside the **pixel** aperture minus the ring stroke — at **production**
grid dims (~30×15), not the 44×18 golden or a cell-space square circle. (Note the
existing truncation `(fy*y_radius) as i32` at `scene.rs:104` quantizes small
vertical roam to 0–1 cells; account for it.)

### 3. Growth ring (open-bottom arc) + rate comet

Replaces the horizontal evolve bar (`draw_hud` step 3 / `hud_evolve_bar_layout`).
Drawn in pixel space against the pixel-space `RoundAperture` already used for the
circular clip.

- **Geometry** — pure helper in **`src/round/`** (`growth_ring_layout`): given
  aperture center, radius, and a bottom-gap angle, return the track arc (gap-left
  → over the top → gap-right) and the fill end for `vm.progress.fraction` (full arc
  when `vm.progress.is_max_stage`; explicit `fraction=0` and `=1` behavior).
- **Painting** (runtime AppKit) — stroke track dim + fill in the existing calm
  violet (`HUD_COLOR_EVOLVE_FILL = (150,120,210)`) via `NSBezierPath` arcs.
- **Rate comet** — a bright dot whose position along the *visible* arc is a pure
  function of an animation phase; it is **hidden across the bottom gap** (where the
  stat lives) and reappears, with no wrap discontinuity on the visible arc. Speed =
  a **nonzero baseline orbit** + a term scaled by `vm.progress.rate_per_hour`
  (`rate_per_hour` is 0 during idle hours, `watch.rs:275-277`; a frozen dot reads
  dead, so the floor keeps it calmly alive). Comet position is a pure, testable
  function of `(phase, ring geometry)`.
- **Animation plumbing** — thread `animation_frame` into the `draw_scene` snapshot
  and keep the view redrawing every UI tick (or fold the comet phase into the
  pet's change detection) so the comet animates during quiet periods, not just when
  the pet moves.
- **No numerals on the ring.** An optional tiny "→ {next_stage_label}" caption near
  the top is allowed but secondary.

### 4. Mood aura

Replaces the three ambient vital ticks (`draw_hud` step 1, `hud_gauge_layouts`,
`GaugeLayout`, the `HUD_GAUGE_*` / `HUD_COLOR_FED|HAPPY|ENERGY` constants — all
removed, along with their unit tests).

- **Color mapping** — pure helper `mood_aura_color(Mood) -> RoundColor` in
  **`src/round/`** (cfg-free, so CI lints/tests it), over all seven `Mood`
  variants (`Happy, Ecstatic, Content, Hungry, Sad, Sleepy, Wilted`). Give **Sad
  and Sleepy distinct hues** (they are distinct needs — happiness<35 vs energy<20)
  rather than collapsing both to one blue-grey. Final palette tuned on-device.
- **Source** — `RoundSceneModel.pet.mood` (confirmed present, full 7-variant enum).
  No new plumbing from the game layer.
- **Follows the pet** — a pixel-space radial gradient drawn *under* the pet at the
  pet's pixel center, before the seam blit. The rendered pet body sits at the drift
  rect **plus** a posture offset (0–1 cell, `colors.rs:177`) and breath offset
  (`vm.breath_offset_y`), applied in `render_pet_to_draw_list` *after* layout. So
  `build_round_scene_draw_list` returns the **rendered** pet rect (post
  posture/breath), and `draw_scene` converts its center to pixels (`cell_to_point`)
  for the gradient. Alignment is exact to the rendered body, not the bare drift rect.

### 5. One clean stat in the ring gap

Keep the token readout (`format_tokens(vm.today_effective_tokens)` +
`format_tokens(vm.progress.rate_per_hour)`):

- Promote a big "today" number with a small "today · {rate}/hr" sub-line.
- **Clamp the text box inside the gap.** "Cannot overlap by construction" holds for
  the ring *arcs*, but the rectangular stat box can still clip the ring near the
  gap edges — so measure the rendered text and constrain it to the gap chord (or
  shrink the font) so the box stays within the open wedge.
- Drop the flanking stage labels (now implied by the ring).

### 6. Tank depth gradient

Replace the flat porthole fill with a subtle radial gradient (lighter center,
darker rim) in `draw_scene` before the seam blit. Cosmetic.

## Module placement & shared-surface impact

- **Pure helpers go in cfg-free `src/round/`** (with `companion_drift`), *not* the
  macOS-gated `src/companion/app.rs`. CI runs `clippy --all-targets -D warnings`
  and `check` **ubuntu-only**, where `app.rs` (`#![cfg(target_os = "macos")]`)
  compiles to nothing — helpers placed there get no CI lint/golden coverage. Only
  `NSBezierPath`/`NSGradient` painting stays in `app.rs`.
- **Scene-builder signature change.** `build_round_scene_draw_list` gains a
  motion-config param and returns a **named struct** (e.g.
  `CompanionScene { draw_list, pet_rect }`), not a bare tuple. This touches all
  callers — companion (`app.rs:495`), menubar (`menubar/render.rs:59`), preview
  (`preview.rs:21`) — which pass default motion config to preserve current behavior.
- **Goldens that move** must be re-blessed: the round-scene content-lock
  (`rasterize.rs:168`) and the four `build_round_scene_draw_list_*` tests
  (`scene.rs:216-271`), `tests/round_draw_list.rs`. Re-verify the menubar render
  still composes.

## Data flow

```
WatchViewModel + RoundSceneModel (mood, vitals, progress, tokens — all existing)
  └─ build_round_scene_draw_list(vm, now, cols, rows, motion_cfg)
        → CompanionScene { draw_list, pet_rect }        [pure, testable]
  └─ draw_scene (macOS), with animation_frame in the snapshot:
        1. tank depth gradient (pixel)
        2. mood aura at pet pixel center (pixel gradient)   ← pet_rect, mood
        3. blit draw_list (pet + props + room)              ← existing
        4. growth ring track + fill + comet (pixel arcs)    ← progress, rate, frame
        5. token stat clamped in ring gap (pixel text)      ← tokens, rate
```

All new *logic* (drift bounds, ring/gap geometry, comet phase, mood color) is pure
and lives in cfg-free `src/round/` helpers. Only `NSBezierPath`/`NSGradient`
painting is runtime AppKit, consistent with the existing seam split.

## Testing

- **TDD for the pure helpers (in `src/round/`):** the bounded-invariant test as
  specified in §2 (box corners → cell→pixel → pixel aperture, production dims);
  ring layout honors the gap and the fill spans `fraction` (incl. 0, 1,
  `is_max_stage`); comet position is continuous on the visible arc and hidden
  across the gap with a nonzero idle floor; `mood_aura_color` covers all seven
  moods with Sad ≠ Sleepy; the stat box fits within the gap chord.
- **Regression / goldens:** re-bless the round content-lock and `scene.rs` /
  `tests/round_draw_list.rs` goldens after the drift retune + signature change;
  remove the vital-tick helpers *and their ~14 unit tests* together so the macOS
  `cargo test` leg stays clean; re-verify the menubar popover composes.
- **Coverage limitation:** the pixel-space HUD (ring, aura, depth gradient,
  repositioned stat) is AppKit-only and absent from the text preview lab — same as
  today's evolve bar. It is verified by (a) unit tests on the geometry/color
  helpers and (b) on-device review on the 960×960 screen. The "bigger pet via fewer
  cols" lever is only partially preview-checkable; the binding bounded check needs
  real device dims.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -D warnings`
  stay clean (locally via lefthook for the macOS code; in CI for the `src/round/`
  helpers).

## Tunables (defaults, tuned on-device)

| Knob | Default | Effect |
|---|---|---|
| `COMPANION_TARGET_COLS` (pet scale) | 36 → ~30–33 | Bigger pet + props + text |
| companion roam fractions (X/Y) | ~0.45 / gentle | Roam range (X-dominant) |
| companion drift period / upward bias | tuned | Liveliness / keep clear of stat |
| ring bottom-gap angle | ~70° | Size of stat gap |
| comet baseline orbit + rate gain | nonzero floor | Always-alive vs. livelier when busy |
| aura palette (7 moods, Sad ≠ Sleepy) | per mood | Mood color |

## Out of scope (YAGNI)

- Surfacing additional unused data (lifetime tokens, age, 7-day trend) — the
  rejected "dashboard" direction.
- Prop count/art/density changes (shared-seam, would touch the watch).
- A pixel-accurate preview of the AppKit HUD; on-device review covers it.
- The "pet drifts in/out of the rim" behavior — explicitly decided against
  (bounded).
- Changing the menubar popover's motion — it keeps current defaults.

## Risks

- **Size↔roam budget** is a hard geometric constraint, not just a feel-tune (only
  ~2 vertical cells at cols≈30). Mitigated by the device-tuned knobs + a default
  that preserves roam, and guarded by the bounded-invariant test.
- **Shared scene builder:** the signature change and any default-config drift must
  leave the menubar/preview/goldens behaving as before; re-verify explicitly.
- **Idle comet:** without the baseline orbit floor, a zero-rate hour freezes the
  dot; the floor is the guard.

## Review provenance

Vetted by a five-lens staff-SWE subagent panel (codebase-fit, render-seam
architecture, geometry/bounded-drift, product/spirit, testing/risk) with each
finding adversarially verified against the source. Verdict: on-track; all
confirmed findings folded in above. Validated as sound and unchanged: the single
size lever, folding vitals into the mood aura, and the pixel/pure render split.
