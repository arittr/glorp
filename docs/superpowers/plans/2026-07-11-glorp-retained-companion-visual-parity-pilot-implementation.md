# Glorp Retained Companion Visual-Parity Pilot - implementation plan

> Historical pilot plan. Execution is superseded by
> docs/superpowers/plans/2026-07-11-glorp-retained-companion-default-cutover-implementation.md.
> Keep this document as the record of the initial parity slice.

> Execute this plan in small slices. Smooth stays default. Routine verification is
> bounded to focused tests and one-shot captures; do not run renderer qualification,
> energy, release, or multi-minute performance matrices during this pilot.

**Goal:** Port the established Smooth companion art and layout semantics to the
hidden retained `wgpu` pipeline, beginning with glyph identity and atlas lifetime.

**Spec:**
`docs/superpowers/specs/2026-07-11-glorp-retained-companion-visual-parity-pilot.md`

## Entry state

The worktree already contains an uncommitted production prototype:

- `retained-renderer` non-default Cargo feature;
- retained companion mode and production command wiring;
- `CAMetalLayer`/`wgpu` host with Smooth fallback;
- translation from `SmoothCompanionScenePlan` into GPU primitives;
- runtime native glyph atlas generation;
- approximate tank gradient, gauge arcs, overlays, and HUD.

Do not discard this work. Refine it in place and keep unrelated dirty files intact.

## Global constraints

- Do not change the default renderer.
- Do not remove or weaken Smooth fallback.
- Do not import `crate::renderer_spike` into production modules.
- Do not add new 2.5D effects, camera behavior, lighting, particles, or materials.
- Do not create a second product-state or pet-art derivation.
- Do not hide unsupported primitives or missing glyphs.
- Do not perform native font lookup/rasterization in ordinary animation frames.
- Do not run long performance or qualification matrices.
- Prefer pure geometry/resource tests over repeated native launches.
- Keep each routine command under approximately two minutes. Stop and narrow any
  command that exceeds the bound.

## Allowed initial file set

- `src/companion/retained.rs`
- `src/companion/retained.wgsl`
- `src/companion/app.rs` only for narrow prepared-data/capture integration
- focused tests in existing modules or `tests/retained_renderer.rs`
- the pilot spec and this plan

A broader module split requires a concrete need discovered during implementation.

## Verification ladder

Use the cheapest sufficient check after each edit:

1. pure/unit test for the changed math or resource policy;
2. narrow compile/test with `--features retained-renderer`;
3. one automatically exiting 360 capture only when pixels need inspection;
4. final one-shot four-size capture set after all parity slices.

Never substitute a long soak for a missing focused assertion.

---

## Task 1: Freeze glyph metric and quad-placement contracts

**Files:**
- Modify `src/companion/retained.rs`
- Add focused unit tests near pure helpers or in `tests/retained_renderer.rs`

Define a testable atlas entry carrying at least:

- normalized ink UV rectangle;
- ink pixel width/height;
- horizontal bearing from the cell origin;
- vertical bearing/baseline placement from the Smooth draw origin;
- raster font size and style identity;
- padding that is excluded from visible UVs.

Add pure helpers that convert a Smooth cell frame plus atlas metrics into a GPU
quad. Cover:

- ordinary glyph;
- narrow glyph;
- descender/baseline case;
- bold style;
- scaled/fractional-motion cell;
- empty background-only cell.

**Gate:** tests demonstrate that different glyph ink bounds produce different
quad bounds and are not stretched to a full cell.

**Bounded checks:** focused test target only; no native launch.

## Task 2: Rasterize and record real glyph metrics

**Files:**
- Modify `src/companion/retained.rs`

Update native atlas compilation so rasterization records actual AppKit font/glyph
measurement rather than assuming a fixed visible inset. Keep deterministic padded
slots, but compute UVs from the recorded ink rectangle. Preserve current font
family, point-size policy, and bold choice used by Smooth.

Handle blank/whitespace glyphs explicitly. Reject an invalid or empty atlas with a
bounded retained failure instead of generating undefined UVs.

**Gate:** atlas entries expose real per-glyph bounds and focused tests cover
packing/UV normalization at slot edges.

**Bounded checks:** focused tests plus narrow `cargo check` with retained feature.

## Task 3: Use metric-aware glyph quads and correct sampling

**Files:**
- Modify `src/companion/retained.rs`
- Modify `src/companion/retained.wgsl` only if required

Replace full-cell glyph rectangles with metric-aware quad placement. Background
fills continue to cover the authored cell. Glyph ink uses its own bounds and
baseline within that cell.

Select a sampling policy that preserves established glyph edges. Start with nearest
sampling for glyph alpha unless a bounded 360 comparison demonstrates that Smooth
requires linear sampling at non-integer depth scales. Do not use one filtering
choice to blur away placement errors.

**Gate:** representative pet glyphs preserve silhouette, eye/mouth alignment, and
cell spacing in a bounded 360 review.

**Bounded checks:** focused tests, narrow compile, one automatically exiting 360
capture or launch. No size matrix.

## Task 4: Cache atlas GPU resources by generation key

**Files:**
- Modify `src/companion/retained.rs`
- Modify `src/companion/app.rs` only if a prepared semantic/layout key must be passed

Introduce a deterministic atlas/resource key derived from the bounded glyph/style
repertoire and relevant raster/layout policy. Retain texture, view, sampler, bind
group, and metadata on `RetainedHost`.

- Compile/upload on first use or key change.
- Reuse unchanged resources on ordinary frames.
- Replace resources atomically after successful compilation.
- Keep the last good atlas when replacement compilation fails, or fall back to
  Smooth when the active frame cannot be represented safely.
- Add counters or test-visible state proving atlas builds/uploads do not increase
  across identical frames.

**Gate:** two identical frame preparations produce one atlas compilation/upload;
a changed glyph repertoire produces exactly one replacement.

**Bounded checks:** pure/resource tests and narrow compile. Do not run CPU or energy
measurement.

## Task 5: Add deterministic bounded parity capture seam

**Files:**
- Modify existing review-capture command/path or add one narrow test helper
- Modify `src/companion/app.rs` only as required
- Add focused capture metadata tests

Provide an automatically exiting, privacy-safe way to render the same deterministic
state through Smooth and Retained at a requested size. It must record renderer,
logical size, backing scale, fixture identity, and output path.

Development default: one 360 Smooth capture and one 360 Retained capture.

**Gate:** one bounded command produces both comparable captures without reading
real user state and exits automatically.

**Bounded checks:** one capture pair. No repeated loop.

## Task 6: Port the dithered tank falloff

Share the existing Smooth interpolation and deterministic dither function through a
backend-neutral helper or implement a pixel-equivalent retained shader/texture.
Do not independently retune colors.

**Gate:** sampled core, midpoint, and rim colors match the shared function within a
declared one-output-level tolerance, and the bounded capture has no visible gradient
banding regression.

## Task 7: Share HUD metrics and layout

Compile HUD glyph runs with measured advances, ink bounds, line heights, and total
bounds outside the frame hot path. Both Smooth and Retained should consume one
backend-neutral prepared layout where practical.

**Gate:** same text produces equivalent centered run bounds and vertical stacking;
no character-count width approximation remains.

## Task 8: Share exact gauge geometry

Replace overlapping-dot arc approximation with shared analytic or tessellated arc
geometry preserving track/fill start/end, stroke width, configured cap style, and
daily overage marker.

**Gate:** focused geometry tests compare retained vertices/parameters with existing
round HUD functions at zero, partial, full, and overage fractions.

## Task 9: Final pilot review

Run once after Tasks 1-8:

- focused retained unit/integration tests;
- formatting and diff checks;
- narrow retained-feature compile;
- one-shot Smooth/Retained captures at 260, 360, 480, and 720;
- bounded retained failure/fallback check.

Do not run:

- five-minute repetitions;
- 12-run matched matrices;
- `powermetrics`;
- full release topology or package qualification;
- broad `cargo test --all-features` unless a concrete dependency change requires it
  and the user approves the expected cost.

Record visual findings and remaining differences. The pilot is not authorization to
flip the default.

## Immediate execution order

Begin now with Tasks 1-4 as one coherent glyph-parity slice. Stop after focused
checks and one representative 360 comparison. Tank, HUD, and gauge work follows
only after the pet art is recognizable.
