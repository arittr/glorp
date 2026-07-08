# Glorp Pixel Cast Identity And Tank Composition - design

- Date: 2026-07-08
- Status: direction approved by Drew; adversarial review corrections incorporated
  before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-07-08-glorp-smooth-pixel-companion-design.md`
  - `docs/superpowers/specs/2026-07-08-glorp-pixel-default-readiness-design.md`
  - `docs/superpowers/measurements/2026-07-08-glorp-pixel-default-readiness-review.md`
  - `docs/superpowers/specs/2026-07-07-glorp-ambient-tank-life-design.md`
  - `docs/superpowers/specs/2026-06-24-glorp-companion-tank-redesign-design.md`

## Problem

Pixel mode now renders a smooth, opt-in pet in the round companion and has a
portable fit/review contract. The remaining product blocker is identity.

The current pixel pet is alive, but it can still read like a generic blocky
creature. The art-reference layer extracts the canonical terminal pet and
already carries per-cell base roles, plus aggregate counts for important
identity cues such as Fuzz lockets, Crystal facets, Glitch repair marks,
appendages, interior texture, and foot contact. That is not yet a strong enough
rendering contract: signature cues and protected regions are not consistently
promoted into renderer-visible cells and privacy-safe review artifacts.

The next slice should make Pixel feel like the real Glorp cast in a living tank.
It should not redesign tank life, add a sprite-sheet pipeline, or flip Pixel to
the default renderer.

## Direction

Build a **pet-first, tank-aware** identity pass.

1. Promote canonical pet-art identity cues into per-cell pixel roles.
2. Teach the Pixel renderer to use those roles for legible species/stage reads.
3. Use already-implemented companion context as composition evidence that proves
   the pet remains identifiable inside the real companion frame.

Tank props and tank life are review context only when their current code paths
already exist. This slice must not implement new tank-life mechanics, new prop
projection behavior, or a live Pixel tank compositor just to satisfy identity
review. New prop or tank behavior belongs in a follow-up after the pet reads
correctly.

## Goals

1. **Cast identity.** All six species produce distinct Pixel frames for the
   representative stages in the review matrix, derived from the canonical pet
   art.
2. **Hero identity gates.** Fuzz S3, Glitch S4, Crystal S5, and Mech S5
   get stricter review because they exercise the cues most likely to be lost:
   locket, repair marks, facets, hard-body silhouette, appendages, and feet.
3. **Role-promoted rendering.** Identity cues are represented as per-cell roles
   or protected regions, not only aggregate counts.
4. **Tank-aware proof.** Preview Lab includes pet-alone and tank-context review
   frames so existing props, existing tank life when available, aura, fit, and
   HUD rules are reviewed together.
5. **Portable acceptance.** The main proof lives in Rust tests and Preview Lab
   artifacts. AppKit screenshots remain useful manual evidence, not the only
   correctness mechanism.
6. **Conservative rollout.** Pixel remains opt-in and Classic remains available.

## Non-goals

- No default renderer flip.
- No removal of Classic.
- No hand-authored sprite sheet or external asset pipeline.
- No new tank-life unlocks, inhabitants, routes, prop catalog entries, or prop
  mechanics.
- No live Pixel tank compositor and no new runtime prop/tank-life avoidance
  system.
- No full redesign of the round tank composition.
- No attempt to make final mascot-quality art for every species/stage
  permutation in one pass.

## Ownership And Seams

The implementation should keep one owner for each layer:

- `presentation::pixel::art_reference` is the only canonical terminal-art
  adapter for typed Pixel contracts.
- `PixelPetArtReference` exposes sanitized, renderer-ready cell roles and
  protected regions. Preview Lab consumes this output; it must not become a
  second canonical-art extractor for typed artifacts.
- The Pixel renderer consumes `PixelPetArtReference` and emits RGBA frames.
- Preview Lab records evidence and review groupings. It does not own canonical
  art decisions and does not mutate live prop or tank-life behavior.
- Tank props and tank life remain in their existing modules. This slice may
  compare their existing cells against Pixel protected regions in Preview
  evidence; it must not push Pixel-specific roles into their placement logic.

The safe flow is:

canonical pet art -> sanitized `PixelPetArtReference` -> Pixel renderer ->
Preview evidence. Tank/prop context can be compared against the sanitized Pixel
regions, but should not depend on raw terminal glyphs or private pet inputs.

## Identity Model

`PixelPetArtReference` should become the identity contract consumed by the Pixel
renderer. It already contains species, stage, mood, pose, occupied cells, bounds,
foot contact, checksum, and role counts. This slice extends that model so
signature cues are actionable.

`PixelArtCell` remains a single exclusive render role unless implementation
evidence proves an additive role model is necessary. Promotion should therefore
use a deterministic priority order for the cell's visible role, while protected
regions and cue coverage are exported as separate evidence.

The visible-role priority is:

1. face roles: `Eye`, `Mouth`
2. signature roles: `Locket`, `Facet`, `RepairMark`
3. existing species/accent roles: `Corruption`, `Pattern`, `Accent`
4. silhouette/grounding roles: `Appendage`, `FootContact`, `Outline`
5. texture/fill roles: `InteriorTexture`, `BodyGlow`, `Body`

The important change is targeted promotion, not a replacement of the existing
reference model:

- Fuzz locket glyph cells become `PixelArtRole::Locket` cells.
- Crystal facet glyph cells become `PixelArtRole::Facet` cells.
- Glitch repair glyph cells become `PixelArtRole::RepairMark` cells.
- Foot-contact cells become `PixelArtRole::FootContact` or a dedicated promoted
  contact region the renderer can emphasize.
- Thin limbs, ears, horns, fins, antennae, or other narrow cells become
  `Appendage` when the canonical footprint supports it.
- Body-edge cells become `Outline`; enclosed body cells become
  `InteriorTexture` or body fill depending on the source role.
- Protected regions are derived from the promoted cell set and represented
  separately from visible roles. At minimum they include face cells and signature
  cue cells used by the hero gates.

Promotion must preserve privacy. It may use canonical rendered glyphs and spans
as an oracle inside `art_reference`, but exported review artifacts must expose
only sanitized roles, bounds, counts, checksums, protected regions, and cue
coverage. They must not expose raw seeds, raw terminal art, source names, usage
counts, absolute/user filesystem paths, diagnostics, prompts, responses, or
transcripts. Relative artifact paths such as `frames/*.json` are allowed because
they are the Preview manifest contract.

## Renderer Behavior

When a valid art reference is available, the renderer should make identity roles
visible in priority order:

1. Face cells: eyes and mouth must remain readable and protected from overlay
   effects.
2. Signature cells: locket, facets, repair marks, and Mech core/hard-body marks
   should be distinct from generic body fill. Mech-specific emphasis should use
   existing `Accent` or `Pattern` roles unless implementation evidence shows a
   dedicated role is necessary.
3. Silhouette cells: outline and appendage roles should preserve species/stage
   shape rather than smoothing everything into a rounded blob.
4. Grounding cells: foot contact should help the pet feel anchored in the tank
   without creating a floor.
5. Texture cells: interior texture should add identity at larger sizes while
   staying quiet at minimum size.

The procedural species body remains an explicit fallback for missing or invalid
references. It should not be the normal path for review fixtures.

Pixel effects must respect the identity contract:

- aura and pulse effects render behind or around protected face/signature cells
- tank life and prop composition cannot occlude protected face regions
- high-alpha body fit continues to honor HUD-safe geometry
- continuous motion does not invalidate the discrete art-reference cache

## Preview Lab Contract

Add a Pixel cast-identity review scenario to Preview Lab. It should produce
artifacts that a reviewer can inspect without launching AppKit.

Required review entries:

- `pixel-fuzz-s3-locket`: hero Fuzz frame with locket cue visible.
- `pixel-blob-s3-body`: representative Blob frame with distinct body read.
- `pixel-ghost-s3-wisp`: representative Ghost frame with distinct silhouette or
  wisp read.
- `pixel-glitch-s4-repair`: hero Glitch frame with repair marks and protected
  face visible.
- `pixel-crystal-s5-facets`: hero Crystal frame with facet cues visible.
- `pixel-mech-s5-hardbody`: hero Mech frame with hard-body silhouette and core
  cue visible.
- `pixel-cast-identity-matrix`: a review grouping/index that references the six
  real Pixel scenario frames above. It is not a multi-frame Pixel artifact and
  it is not allowed to stand in for the rendered frames with labels alone.
- `pixel-tank-composition`: Pixel pet in an existing companion context. It uses
  current prop/tank-life data only if those surfaces already exist in the code
  path; otherwise it records that the unavailable context is deferred.

Required typed artifacts:

- Pixel frame JSON for every review frame.
- Pixel art JSON with promoted role cells, role counts, cue coverage, body
  bounds, face/protected bounds, and checksum.
- Pixel fit JSON using the existing schema `2` geometry evidence.
- Pixel composition JSON for tank-context fixtures, recording protected Pixel
  regions in preview coordinates and comparing them to existing prop/tank-life
  cells when available. This is evidence, not a directive to mutate live
  placement behavior.

The composition artifact should have its own Preview manifest file slot and
artifact type, such as `pixel_composition` / `pixel-composition`. Do not overload
the existing `tank_life` artifact, because tank life remains its own feature
contract.

The Pixel art sidecar should move to schema `2` for this slice. Schema `2`
should be an allowlisted contract containing:

- `role_cells`: sanitized cell coordinates and exclusive visible role names
- `cue_coverage`: expected/present counts for hero cues
- `body_bounds`
- `protected_bounds`
- `signature_regions`
- `foot_contact`
- `reference_checksum`

It must not include raw seed, raw terminal glyph rows, source names, usage
counts, absolute paths, diagnostics, prompts, responses, or transcripts.

The species matrix must not repeat the previous mistake of being mostly a roster
label. It passes only if the manifest/review surface references all six real
Pixel frame artifacts and the HTML review can display those canvases.

Pixel Preview `.txt`, `.cells.json`, and HTML outputs are part of the
privacy-reviewed contract for this slice. They should omit canonical terminal
reference rows. If a human-only terminal reference artifact is still useful, it
must be an explicit local debug artifact outside the default manifest and
outside the privacy-safe review contract.

## Tank Composition Rules

This slice uses existing companion context only. It adds composition evidence
and guardrails only.

Rules:

1. The pet face protected region must not be covered by tank life, foreground
   props, aura, or pulse effects.
2. Signature cues used for hero identity gates must not be covered in tank
   context.
3. Tank life can pass in front of generic body regions, but not through eyes,
   mouth, locket/facet/repair marks, or the HUD-safe text area.
4. Props and tank life should continue to use their current layer semantics.
   If a conflict exists in Preview evidence, record the conflict or defer the
   context. Do not add new live prop/tank-life projection behavior in this slice.
5. The round companion remains a free-float tank. This slice does not add a
   floor, substrate, or new tank mechanics.
6. Product catalog prop/tank-life identifiers and glyph cells may appear in
   fixture-only composition evidence when needed for review, but they must be
   treated as product catalog data, not user/source identifiers. Do not export
   raw pet seeds or user-derived source/activity data in composition artifacts.

## Acceptance Gates

The implementation is not ready to be considered for a future default flip
unless all gates below pass.

1. **Role promotion:** tests prove locket, facet, repair mark, appendage,
   outline/interior, and foot-contact cues are promoted from canonical rendered
   pet output into actionable roles.
2. **Hero renderer impact:** renderer tests prove promoted roles change visible
   pixels, not only metadata.
3. **Species/stage matrix:** Preview Lab renders all six species in representative
   stages and writes typed artifacts for them.
4. **Tank composition:** Preview Lab records pet-in-tank frames and typed
   collision/occlusion evidence for protected regions using only existing
   context. If tank life is unavailable in the current code path, the artifact
   records that status and the gate is limited to existing props/round context.
5. **Privacy:** Pixel art, frame, fit, composition artifacts, `.txt`, and
   `.cells.json` pass an allowlist-style privacy scan. The scan covers schema `2`
   Pixel art fields and any new composition JSON; it must reject raw terminal
   reference rows, raw seeds, source names, usage counts, absolute paths,
   diagnostics, prompts, responses, and transcripts.
6. **Fit/HUD regression:** existing HUD-safe fit tests and geometry evidence
   remain green.
7. **Manual visual review:** Cast identity remains blocked until a reviewer
   approves the identity matrix and tank-context frames.

## Testing

Use a test-first workflow for implementation.

Core tests:

- `tests/pixel_art_reference.rs`: promoted role extraction, cache key stability,
  privacy, and cue coverage.
- `tests/pixel_renderer.rs`: promoted roles affect pixels; hero frames differ by
  real art roles; protected face/signature cells survive effects.
- `tests/dev_preview.rs`: new Preview Lab cast-identity scenario writes frames,
  typed artifacts, tank-context evidence, and privacy-safe outputs. The privacy
  collector must include any new `.pixel-composition.json` or equivalent
  composition artifact, and the scenario tests must assert the six real cast
  frame IDs rather than a label-only matrix.
- `tests/pixel_fit.rs`: existing fit/HUD safety stays green.

Useful commands:

```bash
cargo test --test pixel_art_reference
cargo test --test pixel_renderer
cargo test --features dev-preview --test dev_preview
cargo test --features dev-preview dev_preview::scenarios
cargo test --features dev-preview dev_preview::export
cargo test
```

## Rollout

The result stays opt-in behind the existing Pixel renderer selection. The end
state of this slice is not "Pixel is default"; it is "Pixel identity and tank
composition have reviewable evidence." A separate default-flip decision can only
happen after Cast identity, Resize freshness, CPU, fit, privacy, and AppKit
runtime evidence are all current and approved.
