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
face. It is **companion-local**: no shared watch/seam code changes are required
for the core, so the terminal watch is untouched.

## Validated design

A single composition, **"free-float tank in a growth ring"**:

- **Tank** — unchanged free-float porthole (no floor). A subtle radial depth
  gradient (darker at the rim) so it reads as looking *into* something.
- **Pet** — bigger, and roaming a genuinely wide arc of the tank. Always fully
  on-screen (**bounded** — never clipped by the rim or overlapping the ring/stat).
- **Growth ring** — an open-bottom arc around the rim showing stage progress.
  Felt, not measured: no percentage, no ETA. An orbiting comet on the ring tracks
  live token rate.
- **Mood aura** — a soft radial glow behind the pet, color = mood, that travels
  with the pet. Replaces the invisible vital ticks. Wellbeing is felt through the
  aura color and the pet's eyes (which already shift by mood), not charted.
- **One clean stat** — the token + rate readout, nested in the ring's bottom gap
  so it can never collide with the ring. Big "today" number, small rate sub-line.

This satisfies the brief (bigger pet, nicer stats, bigger props) and honors the
companion-not-optimizer spirit: the ring and aura are ambient, not dashboards.

## Components

### 1. Bigger pet + bigger props — one lever

The pet art is a fixed `PET_W=13 × PET_H=10` cells inside a `COMPANION_TARGET_COLS`
(= 36) grid, so the pet is mechanically locked to ~36 % of the display width. The
props are rendered into the same cell grid through the shared seam, so they scale
with cell size too.

**Change:** lower `COMPANION_TARGET_COLS` (`src/companion/app.rs:606`) from 36 to
~30 (final value tuned on-device). Fewer columns → larger cells → the pet, the
props, and the cell-grid text all grow together. This touches one companion-local
constant and **no shared seam code**, so the watch is unaffected and props get
bigger for free.

Prop *count/placement* is intentionally left alone: changing prop art or density
lives in the shared seam (`src/tui/panels/pet/props.rs`, habitat props) and would
affect the watch. The cell-size lever is expected to be sufficient; if not, prop
tuning is a separate, later decision (out of scope here).

### 2. Pet roams the tank (bounded)

`companion_drift` (`src/round/scene.rs:61`) already eases the pet between
deterministic 2D targets every `DRIFT_PERIOD_SECS` (20 s) via smoothstep. It stays
pure and golden-testable. Retune for "moves around the tank," within a smaller
grid:

- Widen the roam: raise `DRIFT_X_FRAC` (0.45 → ~0.70) and `DRIFT_Y_FRAC`
  (0.30 → ~0.50). Optionally shorten `DRIFT_PERIOD_SECS` for liveliness.
- **Reserve the bottom for the stat:** bias the drift ellipse upward so the pet
  never enters the bottom stat gap. Concretely, offset the ellipse center up by a
  fixed fraction of `safe_y` and/or clamp the lower bound, so the pet body's
  bottom stays above the stat band.
- **Bounded invariant:** the existing clamp keeps the pet inside the grid; add an
  assertion-backed guarantee that, at the tuned fractions, the pet body stays
  within the safe inner circle (never clipped by the aperture or drawn under the
  ring). This is the explicit "bounded, not in/out" decision.

### 3. Growth ring (open-bottom arc) + rate comet

Replaces the horizontal evolve bar (`draw_hud` step 3 and `hud_evolve_bar_layout`).
Drawn in pixel space against the pixel-space `RoundAperture` already used for the
circular clip.

- **Geometry** (pure helper, e.g. `hud_growth_ring_layout`, golden-testable):
  given the aperture center, radius, and a configurable bottom-gap angle, return
  the track arc (gap-left endpoint → over the top → gap-right endpoint) and the
  fill end angle for `vm.progress.fraction` (full arc when
  `vm.progress.is_max_stage`).
- **Painting** (runtime AppKit, not golden-tested, matching existing convention):
  stroke the track arc dim and the fill arc in the calm violet already defined
  (`HUD_COLOR_EVOLVE_FILL = (150,120,210)`), via `NSBezierPath` arc segments.
- **Rate comet:** a small bright dot positioned along the ring by an animation
  phase derived from `animation_frame`, its speed scaled by `vm.progress.rate_per_hour`
  (livelier when busy). Comet position is a pure function of (phase, ring geometry)
  → testable; the dot fill is runtime.
- **No numerals on the ring.** The stage names move off the bar; an optional tiny
  "→ {next_stage_label}" caption near the top of the ring is allowed but secondary.

### 4. Mood aura

Replaces the three ambient vital ticks (`draw_hud` step 1, `hud_gauge_layouts`,
`GaugeLayout`, and the `HUD_GAUGE_*` / `HUD_COLOR_FED|HAPPY|ENERGY` constants —
all removed).

- **Color mapping** (pure helper `mood_aura_color(Mood) -> RoundColor`,
  golden-testable) over the full `Mood` enum (`Happy, Ecstatic, Content, Hungry,
  Sad, Sleepy, Wilted`): e.g. content = teal, happy/ecstatic = warm pink, hungry =
  amber, sad/sleepy = dim blue-grey, wilted = very dim. Final palette tuned
  on-device.
- **Source:** the pet mood already carried on the round scene model
  (`RoundSceneModel.pet.mood`; confirm the exact accessor during planning). No new
  data plumbing from the game layer.
- **Follows the pet:** the aura is a pixel-space radial gradient drawn *under* the
  pet at the pet's pixel center, before the seam grid is blitted. To place it,
  `build_round_scene_draw_list` (`src/round/scene.rs:114`) returns the pet's cell
  rect alongside the `SceneDrawList`; `draw_scene` converts that rect's center to
  pixels (same cell→pixel math as the blit) and draws the gradient there.
  `companion_drift` remains the single source of the pet position, so aura and pet
  always align.

### 5. One clean stat in the ring gap

Keep the existing token readout (`format_tokens(vm.today_effective_tokens)` +
`format_tokens(vm.progress.rate_per_hour)`), but:

- Reposition into the ring's bottom gap (a lower Y, centered on the aperture).
- Promote a big "today" number with a small "today · {rate}/hr" sub-line (two
  lines), instead of one dim line.
- Drop the flanking stage labels (now implied by the ring).

The gap geometry comes from the same `hud_growth_ring_layout` bottom-gap angle, so
the stat band and ring are defined together and cannot overlap by construction.

### 6. Tank depth gradient

Give the porthole background a subtle radial gradient (lighter center, darker rim)
in `draw_scene` before the seam blit, replacing the flat fill. Cosmetic; pure
color choice.

## Data flow

```
WatchViewModel + RoundSceneModel (mood, vitals, progress, tokens — all existing)
  └─ build_round_scene_draw_list(vm, now, cols, rows)
        → (SceneDrawList, pet_cell_rect)            [pure, testable]
  └─ draw_scene (macOS):
        1. tank depth gradient (pixel)
        2. mood aura at pet pixel center (pixel gradient)   ← pet_cell_rect
        3. blit SceneDrawList (pet + props + room)          ← existing
        4. growth ring track + fill + comet (pixel arcs)    ← progress, rate, frame
        5. token stat in ring gap (pixel text)              ← tokens, rate
```

All new *logic* (drift bounds, ring geometry, comet phase, mood color, gap
geometry) is pure and lives in golden-testable helpers. Only `NSBezierPath` /
`NSGradient` painting is runtime AppKit, consistent with the existing seam split.

## Testing

- **TDD for the pure helpers:** drift stays within the safe inner circle and above
  the stat band at the tuned fractions; ring layout endpoints honor the bottom gap
  and the fill spans `fraction`; comet position is continuous along the arc;
  `mood_aura_color` covers all seven moods; stat gap and ring never overlap.
- **Preview lab:** the cell-space changes (bigger pet via fewer cols, wider roam)
  are exercisable through the existing round preview. **Limitation:** the
  pixel-space HUD (ring, aura, depth gradient, repositioned stat) is AppKit-only
  and does not appear in the text preview — same as the current evolve bar/vital
  ticks. These are verified by (a) unit tests on the layout helpers and (b)
  on-device review on the 960×960 screen.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -D warnings`
  must stay clean. Removing the vital-tick helpers/constants must not leave dead
  code that trips the all-targets gate.

## Tunables (defaults, all tuned on-device)

| Knob | Default | Effect |
|---|---|---|
| `COMPANION_TARGET_COLS` | 36 → ~30 | Bigger pet + props + text |
| `DRIFT_X_FRAC` / `DRIFT_Y_FRAC` | 0.45/0.30 → ~0.70/0.50 | Roam range |
| drift upward bias | new | Keeps pet out of the stat gap |
| ring bottom-gap angle | ~70° | Size of stat gap |
| comet rate scaling | — | Orbit speed vs. token rate |
| aura palette | per mood | Mood color |

## Out of scope (YAGNI)

- Surfacing additional unused data (lifetime tokens, age, 7-day trend) — that was
  the rejected "dashboard" direction.
- Prop count/art/density changes (shared-seam, would touch the watch).
- A pixel-accurate preview of the AppKit HUD; on-device review covers it.
- The "pet drifts in/out of the rim" behavior — explicitly decided against
  (bounded).

## Risks

- **Roam vs. size tension:** fewer columns shrink the roam grid while the pet
  grows. The tuned `DRIFT_*_FRAC` must keep motion lively without clipping. The
  bounded invariant test guards correctness; the *feel* is an on-device tune.
- **Aura/pet alignment:** both must derive from the same `companion_drift`
  position. Returning the pet rect from the scene builder (rather than recomputing)
  removes the chance of drift.
