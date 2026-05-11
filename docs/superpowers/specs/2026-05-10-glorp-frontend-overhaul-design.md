# Glorp Frontend Overhaul

Date: 2026-05-10

## Overview

Rewrite glorp's frontend in two coupled directions: (1) replace the authored pet-art template system with procedural Unicode generation rendered through Braille bitmap composition, so every pet has a structurally unique silhouette and a unique evolution trajectory; (2) replace the custom char-counting composition layer (`src/tui/composer.rs` and the bulk of `src/tui/layout.rs`) with native ratatui `Layout` + `Flex` primitives organised behind a small `Panel` trait. Layer cinematic state transitions on top via `tachyonfx`, introduce a single owner for pet animation state (`PetAnimator`), and add mouse-tracked eyes.

The overhaul stays on ratatui. iocraft, lipgloss-rs, ratatui-image, and authored spritesheets are explicitly rejected (see Alternatives Considered). Net code size shrinks by ~700 LOC while capability grows substantially.

## Relationship to other work

This spec is the follow-up the `2026-05-10-glorp-watch-visual-redesign-design.md` spec foreshadowed when it carried forward the pet-art system unchanged. The watch visual redesign reworked outer chrome, sparkline, gradient bars, today/feed/helpers content, and the wide/compact split. That work is consumed by the new `Panel` trait — the rebuilt panels render the same content with the same gradients, just decoupled from `composer.rs` and assembled via native `Layout` instead of hand-padded spans.

`WatchViewModel` is the API boundary between view and data; this spec does not touch it. `commands/watch.rs` and the `UsageProvider` chain are unchanged.

## Goals

- Every pet has a structurally unique silhouette generated from its seed, not just unique fills inside a fixed template.
- Every pet evolves along a unique trajectory: stage transitions shift silhouette parameters along a seeded mutation vector, not just swap to a fixed s1/s2 template.
- The pet feels alive between events: idle breathing at sub-character resolution, blink, mood-driven color drift, mouse-tracked eyes.
- State transitions (hatch, stage-up, mood change, feed pulse, low-energy droop) are cinematic, not instant glyph swaps or string flashes.
- Delete the custom char-counting composition layer. Layout is expressed in native ratatui primitives.
- Net LOC decreases. Code boundaries are clearer — animation, generation, rendering, layout, and panel content each live in one place.

## Non-Goals

- No iocraft or any other replacement TUI framework. Ratatui stays.
- No image protocols (Kitty/Sixel/iTerm2). The pet is text glyphs end to end. ratatui-image is not adopted.
- No authored spritesheets, no authored body-part library. Generation is algorithmic.
- No game-mechanics changes — calibration, evolution thresholds, decay, ingestion, persistence are out of scope.
- No CLI surface changes to non-watch commands (`init`, `status`, `doctor`, `rename`, `reset`).
- No back-compat shim for the old pet-art format. Saved pets retain their seed; the seed regenerates them under the new system. Visible to the user as a one-time look change, identity preserved.

## Pet generation

### Blueprint

A pet's seed deterministically produces a `PetBlueprint`:

```rust
struct PetBlueprint {
    species: Species,
    stage: Stage,
    silhouette: SilhouetteParams,
    palette: PaletteRoles,                // existing OkLCH role assignment
    feature_anchors: FeatureAnchors,      // where eyes/mouth/ornaments attach
    feature_glyphs: FeatureGlyphSet,      // glyphs picked from stage-appropriate subsets
    mutation_vector: MutationVector,      // delta applied to SilhouetteParams at each stage-up
}

struct SilhouetteParams {
    width_px: u8,            // pixel grid width (always even — Braille is 2 wide per cell)
    height_px: u8,           // pixel grid height (multiple of 4 — Braille is 4 tall per cell)
    roundness: f32,          // 0..1 — Gaussian envelope tightness
    taper: f32,              // 0..1 — corner falloff strength
    body_density: f32,       // 0..1 — overall fill probability
    asymmetry_seed: u32,     // drives one-off asymmetric ornaments (single antenna, etc.)
    head_zone_ratio: f32,    // 0..1 — fraction of top grid reserved as head
    ornament_density: f32,   // 0..1 — additive features beyond core silhouette
}
```

Stage sizes (pixels → braille char dimensions):
- s0: 14×8 px → 7×2 braille cells
- s1: 18×12 px → 9×3 braille cells
- s2: 22×16 px → 11×4 braille cells

### Silhouette algorithm

1. Sample a 2D field over a half-grid (left side only). For each pixel `(x, y)`, compute fill probability `p(x, y)` as:
   - A Gaussian envelope centred on a body anchor, scaled by `roundness`.
   - Multiplied by a head-zone gain (cells in the top `head_zone_ratio` get a boost).
   - Multiplied by a corner falloff `(1 - distance_to_corner ** taper)` so silhouettes are never rectangular.
   - Multiplied by seeded coherent noise (one octave) to perturb the envelope so two pets with the same overall shape still differ in detail.
2. Threshold `p(x, y) > body_density` → boolean fill. Reject samples where total filled pixels fall below a per-stage minimum; re-sample with adjusted density up to a small retry cap.
3. Mirror the left half to the right half. Bilateral symmetry is guaranteed for the core silhouette.
4. Add ornaments as a separate overlay layer (the core silhouette stays symmetric). The `asymmetry_seed` drives 0–2 asymmetric ornaments (single antenna, side curl) and 0–`ornament_density × stage_max` symmetric ornaments (matched antennae, fin pairs). Ornaments attach to the silhouette edge, never replace body pixels. Asymmetric ornaments are bounded so the pet still reads as a coherent creature — a single fin on one side, not a chaotic protrusion field. Ornaments are pixel patterns drawn from a small algorithmic catalogue (rectangles, hooks, dots), not authored art.
5. Reserve eye-anchor cells. The head zone always contains two anchor positions (one per eye) that body pixels are forbidden from filling. These anchors are where feature glyphs overlay during rendering.

### Aesthetic biases ("stay cute" constants)

Tunable constants in `pet/generate.rs::AESTHETIC`. These are **starting values for the Phase 0 spike**, not final — every constant is expected to shift during tuning. Final values land with the Phase 1 implementation.

- Symmetry: always (load-bearing, not tuned).
- `MIN_ROUNDNESS = 0.45` — prevents stringy or fragmented bodies.
- `MAX_TAPER = 0.75` — prevents pinched corners that look broken.
- `HEAD_ZONE_MIN_RATIO = 0.30` — ensures a recognisable head region.
- `MIN_FILLED_PIXELS_RATIO = 0.35` — rejects sparse silhouettes (lower bound; real creatures rarely fill more than ~50% of their bounding box).
- `MAX_ORNAMENT_DENSITY[stage]` — `[0.10, 0.25, 0.45]` for s0/s1/s2.
- `EYE_ANCHOR_RESERVATION = 2×4 px` per eye (one full Braille character cell) — body pixels cannot occupy these cells, so eye glyphs always have a clean canvas (load-bearing, not tuned).

### Per-species variation

Each species supplies an override block for the aesthetic constants and a glyph-subset bias. Examples (concrete subsets defined during implementation):
- Blob: high roundness, low ornament density, eyes biased to `• o ●`.
- Mech: low roundness, high symmetry strictness, ornament catalogue biased to box-drawing glyphs (`╭ ╮ ╰ ╯`), eyes biased to geometric glyphs (`◇ ◆ ▣`).
- Ghost: tall aspect ratio, jittered baseline, eyes biased to `· ° ʘ`.

Species is the *only* hand-authored degree of freedom in pet generation. Within a species, all variation is procedural.

### Mutation vector and evolution

Each pet's `MutationVector` is a stable seeded perturbation of `SilhouetteParams`:

```rust
struct MutationVector {
    d_roundness: f32,
    d_taper: f32,
    d_body_density: f32,
    d_ornament_density: f32,
    d_head_zone_ratio: f32,
}
```

At each stage-up, the new stage's `SilhouetteParams` is computed by adding the mutation vector to the previous stage's params (clamped to per-stage bounds). Two pets that hatched as similar blobs at s0 can diverge meaningfully by s2.

Feature glyph subsets also expand by stage — s0 picks from a narrow alphabet, s2 picks from a richer one. Evolution shows in both silhouette and feature richness.

### Rendering

The pet bitmap is rendered to Braille at 2×4 px per char. Feature glyphs (eyes, mouth, ornaments anchored to specific positions) replace the underlying Braille cell at their anchor coordinates with a single Unicode glyph from the appropriate stage subset.

Per-cell palette role assignment preserves the existing `StyledSegment` model: each output cell carries a `PaletteRoleName` based on its source region (head zone → `Eye`/`Body`/`Accent`, ornament cells → `Accent` or `Pattern`, body interior → `Body`, mouth/jaw region → `Mouth`).

The static (pre-animation) renderer output stays in the existing `RenderedPet { lines: Vec<String>, spans: Vec<StyledSegment> }` shape. Phase 3 wraps this in `PetFrame`, which adds `overlay_effects: Vec<EffectHandle>` for active tachyonfx layers. Phases 1–2 use `RenderedPet` directly; Phase 3 onward, `PetPanel` consumes `PetFrame` and treats its `lines`/`spans` fields exactly like today's `RenderedPet`.

### Aesthetic validation gate

Phase 0 of the migration ships a standalone `examples/pet_gallery.rs` binary that generates 50 pets across species × stages and prints them in a grid. The gate has two criteria; both must pass:

1. **Zero visually broken pets** in a sample of 50. Broken means: disconnected blobs, sub-minimum body area, missing eye anchors, sharp rectangular outlines, or any silhouette that doesn't read as a single creature.
2. **≥85% read as intentional creatures** (cute or characterful) on visual inspection. The remaining ≤15% may read as "weird but plausible" — odd but still creature-shaped.

If criterion 1 fails, fix the algorithm or rejection rules until it passes — broken pets are not negotiable. If criterion 2 fails after a tuning pass, the fallback is compositional parts: a small authored part library (heads, bodies, eye-pairs, accents) procedurally composed. The rest of this design (animation, layout, transitions) is unaffected by which generation strategy lands.

## Pet animation

A new `PetAnimator` in `src/pet/animator.rs` is the single owner of per-tick pet animation state.

```rust
struct PetAnimator {
    blueprint: PetBlueprint,
    tick: u64,
    breath_phase: f32,
    blink_state: BlinkState,
    mood: Mood,
    active_transitions: Vec<TransitionFx>,  // tachyonfx effects currently playing
    pending_events: VecDeque<PetEvent>,     // Hatch, StageUp, Mood, Feed, LowEnergy
    cursor_hint: Option<CursorHint>,        // from mouse-tracked eyes
}

// Normalized cursor x relative to the pet panel: -1.0 (left edge) … 0.0 (center) … +1.0 (right edge).
// `None` when the cursor is outside the panel or mouse tracking is disabled.
type CursorHint = f32;

enum PetEvent { Hatch, StageUp { from: Stage, to: Stage }, Mood(Mood), Feed, LowEnergy(f32) }

impl PetAnimator {
    fn tick(&mut self, now: Instant) -> PetFrame { ... }
    fn enqueue(&mut self, event: PetEvent) { ... }
    fn set_cursor_hint(&mut self, hint: Option<CursorHint>) { ... }
}

struct PetFrame {
    lines: Vec<String>,
    spans: Vec<StyledSegment>,
    overlay_effects: Vec<EffectHandle>,     // tachyonfx layers to apply during render
}
```

### Layers

1. **Idle breathing** (no library). Each tick, the silhouette is regenerated with `breath_phase`-modulated `height_px` and `roundness` (within ±1 px / ±0.05 of the canonical params). Frequency = `breath_period` from the existing `AnimationProfile`. Reads as actual breathing because Braille gives sub-character resolution.
2. **Blink** (feature glyph swap). Eye overlay glyphs cycle through a 3-glyph sequence over ~3 ticks at `blink_average` frequency (existing `AnimationProfile` field).
3. **Mood** (gradual color shift + feature swap). On `PetEvent::Mood`, eye/mouth glyphs swap to mood-specific glyphs immediately, and a tachyonfx `hsl_shift` runs on the body palette over ~300 ms to drift saturation/lightness toward the mood target (sleepy: cooler+dimmer, happy: warmer+brighter, sad: desaturated).
4. **State transitions** (tachyonfx). Four events get cinematic treatment:
   - **Hatch** (~1.2 s): three phases — egg silhouette `coalesces` from noise, cracks appear via radial sweep + jitter, pet bitmap `dissolves` in.
   - **Stage-up** (~800 ms): old bitmap held for ~200 ms with a brightening flash, then `evolve` transition (chars morph through random intermediates) into the new bitmap, then `sweep_in` settle.
   - **Feed pulse** (~400 ms): left-to-right `wave` highlight across the body; a small floating `+` particle drifts up from the mouth for ~6 ticks via the existing particle plumbing.
   - **Low-energy droop** (continuous): `darken` + `desaturate` applied with intensity `1 - energy_fraction`. Visible over hours of inactivity.
5. **Mouse-tracked eyes** (crossterm mouse events, no library). When `glorp watch` receives a `MouseEvent` whose coordinates fall within the pet panel's rect, the eye overlay glyph swaps based on cursor x relative to pet center (`< <`, `o o`, `> >`). Falls back to the neutral glyph when the cursor leaves the area. Requires `EnableMouseCapture` at terminal init, guarded by capability check. A new watch-mode key `m` toggles mouse tracking at runtime so users in tmux or terminals where mouse capture interferes with text selection can disable it without leaving the app. (Y-axis tracking is a future possibility but out of scope for this overhaul — keep the feature minimal until we see how it lands.)

### Render integration

The watch app calls `animator.tick(Instant::now())` once per render frame and receives a `PetFrame`. The `PetPanel` consumes `PetFrame.lines` + `PetFrame.spans` exactly the way it consumes `RenderedPet` today. `PetFrame.overlay_effects` is applied via tachyonfx after the panel paints its base content.

The animator is unit-testable without ratatui — `tick()` returns a value, `enqueue()` mutates state, neither touches I/O.

## Layout overhaul

### Panel trait

```rust
trait Panel {
    fn min_height(&self, width: u16) -> u16;
    fn preferred_constraint(&self) -> Constraint;
    fn render(&self, area: Rect, buf: &mut Buffer, vm: &WatchViewModel);
}
```

Concrete panels: `PetPanel`, `VitalsPanel`, `TodayPanel`, `SparkPanel`, `FeedPanel`, `HelpersPanel`. Each owns its rendering, its preferred constraint, and its honest minimum height. Each lives in its own file under `src/tui/panels/`.

### Frame composition

```rust
fn render(frame: &mut Frame, vm: &WatchViewModel) {
    let outer = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(frame_title(vm))
        .title_bottom(frame_footer())
        .style(styles().outer_frame);
    let inner = outer.inner(frame.area());
    frame.render_widget(outer, frame.area());

    let mode = if inner.width >= COMPACT_THRESHOLD { Mode::Wide } else { Mode::Compact };
    let panels = build_panel_layout(mode, vm);
    layout_and_render(inner, &panels, frame.buffer_mut(), vm);
}
```

`build_panel_layout(mode, vm)` returns an ordered list of `(Box<dyn Panel>, PanelPosition)` where `PanelPosition` indicates wide-mode column (Left or Right) or compact-mode order. Adding a new panel = one match-statement edit.

`layout_and_render` walks the list, building a ratatui `Layout` per column with each panel's `preferred_constraint()` and `Flex::Start`, then renders each into its allocated rect. Inter-panel gaps come from `Layout::spacing(1)`, not hand-padded blank rows.

### Wide mode (≥104 cols)

Horizontal layout: `Layout::horizontal([Length(40), Length(4), Min(0)])` with `Flex::Start` — pet column, gutter, main column. Inside each column, a vertical `Layout` with `spacing(1)` and per-panel constraints.

### Compact mode (<104 cols)

Single vertical `Layout` with all panels stacked in priority order. Lower-priority panels use `Constraint::Min(0)` so the layout solver can collapse them when the terminal is short. Each panel's `min_height` is honest; the solver does the rest.

Below ~10 rows, the dispatcher falls back to a minimal rendering: pet art + single-line vitals summary (`fed N · happy N · energy N · xp N`). Below 4 rows, vitals summary only. The renderer must not panic at any height ≥ 1.

### Styling and chrome

- Outer rounded border via `Block::bordered().border_type(BorderType::Rounded)`. Replaces `box_with_chrome`.
- Inner padding via `Block::padding(Padding::new(2, 2, 1, 1))`. Replaces manual 4-space pads.
- Section dividers become `Block::default().borders(Borders::TOP).title(format!(" {} ", label))` on each downstream panel — the title sits on the top border line and reads as `─── vitals ───`. Replaces `section_divider()`.
- `Stylize` trait used throughout for style construction (`"foo".bold().fg(color)`), in preference to manual `Style::default().fg(...).add_modifier(...)` chains.
- Existing `SemanticStyles` struct retained as-is — verbose but clear. Existing bar gradients (`BAR_RAMP_GOOD`, `BAR_RAMP_ACCENT`) retained.
- Adaptive palette (optional, Phase 6 polish — see Migration sequence): a single `terminal-light` query at app init picks dark vs light. Today's hardcoded dark palette becomes the dark variant; a light variant is authored using the same OkLCH role structure with inverted lightness anchors. The query is a one-shot; no re-detection. Dark ships unconditionally; the light variant ships only if it can be authored to the same aesthetic quality as the dark one.
- Frame footer copy updates with Phase 5 to `q quit · r refresh · m mouse · ? help`. When mouse tracking is disabled (terminal capability or `m`-toggled off), the `m mouse` token greys out via `.dim()` instead of disappearing — keeps the footer width stable.

### Deletions

- `src/tui/composer.rs` (323 LOC) — entirely removed. `pad_row`, `join_horizontal_top`, `box_with_chrome`, `section_divider`, `split_after_width` all replaced by native primitives.
- The bulk of `src/tui/layout.rs` (~800 of 1133 LOC) — the wide/compact dispatcher, the `LEFT_COL`/`RIGHT_COL`/`GUTTER` constants, the body-row builder, the `body_height` backfill logic, and the per-panel char-precision span math.
- `pet/art.rs` (630 LOC) of authored templates — replaced by procedural generation.
- The duplicate animation logic scattered across `app.rs` and `view_model.rs` — moved to `PetAnimator`.

### Frame fill behaviour

The recent commits oscillating frame height all become moot. The outer block fills `frame.area()`. The inner `Layout` uses `Flex::Start` so panels pack to the top; trailing space is empty buffer. No backfill, no body-height accounting, no shrink-to-natural-height debate.

### Snapshot tests

Existing `render_wide_tests` / `render_compact_tests` use `TestBackend` + buffer assertions. They mostly survive — they test the rendered buffer, not composer internals. Tests that assert specific character widths of composer functions get rewritten against panel boundaries. Expect ~30% test churn.

## Migration sequence

Each phase is independently shippable. Order:

### Phase 0 — Procedural pet spike (gate)

New `examples/pet_gallery.rs`, standalone, no ratatui. Generates and prints 50 pets across species × stages. Iterate on `AESTHETIC` constants until both gate criteria pass (see Pet generation → Aesthetic validation gate): zero broken pets, ≥85% read as intentional creatures.

**Gate decision:** if both criteria pass, proceed to Phase 1. If only criterion 1 (no broken) passes but criterion 2 falls short after tuning, replace the generation algorithm with compositional parts (small authored part library + procedural composition) before proceeding. Phases 2–6 are not affected by which generation strategy lands.

### Phase 1 — Pet rendering replacement

- Move spike code into `src/pet/generate.rs`.
- Delete `src/pet/art.rs`.
- Rewrite `src/pet/render.rs` to consume `PetBlueprint` → braille bitmap + overlays → `RenderedPet`. Per-cell `StyledSegment` model preserved.
- Existing saved pets retain their seed; seeds re-generate under the new system. One-time visible look change.
- New snapshot tests for the generator (deterministic per seed).

### Phase 2 — Layout overhaul

Independent of Phase 1 — `PetPanel` is defined against the `RenderedPet` shape, which is stable across the old (authored template) and new (procedural) generators. If Phase 2 ships first, `PetPanel` renders today's pet art; when Phase 1 lands, the same panel renders generated pets without code change.

- Introduce the `Panel` trait + `src/tui/panels/` module.
- Migrate panels one at a time, simplest first: `PetPanel`, `VitalsPanel`, `TodayPanel`, `SparkPanel`, `FeedPanel`, `HelpersPanel`. Each migration is its own commit. App stays working between commits.
- Swap `render_watch_frame` for `build_panel_layout` + `layout_and_render`.
- Delete `composer.rs`. Update snapshot tests.

### Phase 3 — Animation orchestration

- Introduce `PetAnimator` in `src/pet/animator.rs`.
- Move per-tick animation logic out of `app.rs` and `view_model.rs` into the animator.
- Watch loop calls `animator.tick()` once per frame; result feeds `PetPanel`.
- Introduce the two-rate scheme: idle stays at 250 ms; when the animator reports `active_transitions` or in-flight breath/blink, the watch loop ticks at ~16 ms (60 fps target) until quiescent. `WatchAppConfig` exposes both rates so tests can pin them. Measure CPU during a synthetic stage-up before declaring done; fall back to 33 ms (30 fps) if needed.
- No new visible behaviour from the refactor itself, but the higher burst rate is observable as smoother blink and breath.

### Phase 4 — Tachyonfx transitions

- Add `tachyonfx` to `Cargo.toml`.
- Hatch sequence first (visible in `glorp init`, easy to QA).
- Stage-up next (wired to existing stage progression event).
- Mood fade and feed pulse next.
- Low-energy continuous droop last.
- Each transition is its own commit + unit test.

### Phase 5 — Mouse-tracked eyes

- Enable `EnableMouseCapture` in terminal init, guarded by capability check.
- Mouse event handling in the watch loop.
- Pet-area hit-testing using the `PetPanel` rect.
- Eye glyph selection based on cursor x relative to pet center; neutral fallback when cursor leaves.
- New watch-mode key `m` toggles mouse tracking; update `?` help and the frame footer to surface it.

### Phase 6 — Polish

- `terminal-light` adaptive palette query at init.
- `Stylize` trait migration across remaining styled-text construction.
- Snapshot test refresh.

### Rough LOC budget

| Phase | New | Deleted | Net |
| ----- | --- | ------- | --- |
| 0 spike | 200 (example) | 0 | +200 |
| 1 pet | ~500 | ~1260 (art.rs + most of render.rs) | -760 |
| 2 layout | ~800 (panels) | ~1450 (composer + layout.rs core) | -650 |
| 3 animator | ~300 | ~150 (app/view_model anim) | +150 |
| 4 tachyonfx | ~250 (transition wiring) | 0 | +250 |
| 5 mouse | ~80 | 0 | +80 |
| 6 polish | ~50 | ~30 | +20 |
| **Total** | **~2180** | **~2890** | **~-710** |

## Alternatives considered

**iocraft (React-style TUI framework over ratatui).** Rejected. Glorp's identity lives in the pet, and the pet-quality libraries — `tachyonfx` for transitions, `ratatui-image` for image protocols — are ratatui-only. Adopting iocraft forfeits both. iocraft also has open flicker issues (#117) that the 250 ms tick loop would expose, a single maintainer, and a small ecosystem with limited training material. The DX win iocraft offers over native ratatui `Layout::Flex` is real but smaller than the cost of losing the ratatui ecosystem.

**lipgloss-rs (faithful Lipgloss port).** Rejected. Renders to ANSI strings, not directly into a ratatui buffer. The integration friction (route through `ansi-to-tui`) is not worth it when native ratatui `Stylize` + `Block::padding` + `Layout::spacing` covers the same ground for our needs.

**ratatui-image (Kitty/Sixel/iTerm2 pixel pets).** Rejected. Procedural uniqueness with authored sprites would require generating PNG bitmaps per pet at runtime — significant engineering for marginal aesthetic gain when Braille already provides sprite-level fidelity in pure text. Also has tmux compatibility issues many users would hit.

**Compositional parts (authored part library + procedural composition).** Held in reserve as the Phase 0 fallback. Not the default because hand-authoring parts per species contradicts the "minimal authoring, maximum variation" requirement.

**Algorithmic silhouette with ASCII (no Braille).** Rejected. ASCII at 1 glyph per cell gives too little resolution for procedural shapes to read as intentional creatures; results look chaotic. Braille's 2×4 sub-character grid is what makes procedural generation viable.

**Multi-frame ASCII animation (per-frame templates).** Rejected. Subsumed by Braille bitmap regeneration — breathing happens at the pixel level for free each tick. No need for explicit per-frame templates.

## Risks

1. **Procedural cuteness.** The Phase 0 gate exists because procedural generation can produce visually weird creatures. Mitigation: explicit aesthetic biases (symmetry, roundness, head/body structure, eye anchors, taper), spike before committing, fallback to compositional parts if the gate fails.
2. **Per-cell palette role assignment for procedural bitmaps.** Today's role assignment is authored per template cell. The new generator must assign roles algorithmically by region (head zone → eye/body, ornament cells → accent, body interior → body, mouth region → mouth). Risk that automatic assignment produces visually muddy color regions. Mitigation: per-species role-region rules tuned during spike.
3. **Tachyonfx coexistence with the buffer-level rendering ratatui uses.** Tachyonfx is now a ratatui-org crate so the integration is supported, but glorp's snapshot tests assert on buffers. Transitions are temporal — their assertions need to be against discrete intermediate frames, not state-free. Mitigation: snapshot tests run the animator at fixed tick offsets and assert the buffer at each.
4. **Mouse capture compatibility.** `EnableMouseCapture` works in most terminals but interferes with native text selection (most visible in tmux). Mitigation: enable by default where the terminal supports it, expose a new watch-mode key `m` to toggle mouse tracking at runtime, and document the toggle in `?` help.
5. **Existing pets look different after migration.** Each saved pet's seed regenerates a different-looking creature. This is a one-time visible change. Mitigation: communicate clearly in release notes ("your pet got a glow-up; same name, same age, same trajectory, new look").
6. **Spike scope creep.** Phase 0 must remain a throwaway. Mitigation: hard cap at ~200 LOC, no ratatui dependency, no integration with existing modules. The output is a printed grid plus a go/no-go decision, nothing else.
7. **Tick rate vs animation smoothness.** Today's loop ticks at 250 ms (~4 fps). Tachyonfx transitions and braille-pixel breathing both look choppy at that rate; the library's own examples run at 30-60 fps. Mitigation: in Phase 3 (Animation orchestration), introduce a two-rate scheme — idle tick stays at 250 ms when nothing animates, and a higher render rate (~16 ms / 60 fps target, capped to terminal redraw capability) engages while any `active_transitions` exist or while breath/blink is in-progress. The poll worker thread is unaffected. Measure CPU during a stage-up to confirm the burst rate is acceptable; if not, fall back to 33 ms / 30 fps during transitions.

## Open questions

- Tachyonfx version pin and feature flags — defer until Phase 4 implementation begins; pin to whatever's current then.
- Exact glyph subsets per stage and species — finalised during Phase 0 spike, captured in `pet/generate.rs::SPECIES`.
- Stage-up transition memory model — the morph effect needs both the previous-stage bitmap and the new-stage bitmap held simultaneously for ~800 ms. Confirm during Phase 4 whether `PetAnimator` holds them inline or whether tachyonfx's `evolve` primitive takes ownership.
