# Glorp Pixel Cast Identity And Tank Composition - design

- Date: 2026-07-08
- Status: direction approved by Drew; written for review before implementation
  planning
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
creature. The art-reference layer extracts the canonical terminal pet and counts
important identity cues such as Fuzz lockets, Crystal facets, Glitch repair
marks, appendages, interior texture, and foot contact. Those cues are not yet a
strong enough rendering contract: several are aggregate counts rather than
per-cell roles the pixel renderer can color, protect, and prove in context.

The next slice should make Pixel feel like the real Glorp cast in a living tank.
It should not redesign tank life, add a sprite-sheet pipeline, or flip Pixel to
the default renderer.

## Direction

Build a **pet-first, tank-aware** identity pass.

1. Promote canonical pet-art identity cues into per-cell pixel roles.
2. Teach the Pixel renderer to use those roles for legible species/stage reads.
3. Use existing tank props and ambient tank life as composition fixtures that
   prove the pet remains identifiable inside the real companion context.

Tank props and tank life are the review environment for this slice, not the main
feature. New prop behavior belongs in a follow-up after the pet reads correctly.

## Goals

1. **Cast identity.** All six species produce distinct Pixel frames for the
   representative stages in the review matrix, derived from the canonical pet
   art.
2. **Hero identity gates.** Fuzz S3, Glitch S4, Crystal S5, and late-stage Mech
   get stricter review because they exercise the cues most likely to be lost:
   locket, repair marks, facets, hard-body silhouette, appendages, and feet.
3. **Role-promoted rendering.** Identity cues are represented as per-cell roles
   or protected regions, not only aggregate counts.
4. **Tank-aware proof.** Preview Lab includes pet-alone and tank-context review
   frames so props, ambient tank life, aura, fit, and HUD rules are reviewed
   together.
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
- No full redesign of the round tank composition.
- No attempt to make final mascot-quality art for every species/stage
  permutation in one pass.

## Identity Model

`PixelPetArtReference` should become the identity contract consumed by the Pixel
renderer. It already contains species, stage, mood, pose, occupied cells, bounds,
foot contact, checksum, and role counts. This slice extends that model so
signature cues are actionable.

The important change is promotion:

- Fuzz locket glyph cells become `PixelArtRole::Locket` cells.
- Crystal facet glyph cells become `PixelArtRole::Facet` cells.
- Glitch repair glyph cells become `PixelArtRole::RepairMark` cells.
- Foot-contact cells become `PixelArtRole::FootContact` or a dedicated promoted
  contact region the renderer can emphasize.
- Thin limbs, ears, horns, fins, antennae, or other narrow cells become
  `Appendage` when the canonical footprint supports it.
- Body-edge cells become `Outline`; enclosed body cells become
  `InteriorTexture` or body fill depending on the source role.

Promotion must preserve privacy. It may use canonical rendered glyphs and spans
as an oracle while extracting the reference, but exported machine-readable Pixel
artifacts must expose sanitized roles, bounds, counts, checksums, and cue
coverage. They must not expose raw seeds, raw terminal art as the runtime API,
source names, usage counts, paths, diagnostics, prompts, responses, or
transcripts.

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

Required still frames:

- `pixel-cast-identity-matrix`: all six species across representative stages,
  using actual Pixel frames rather than text labels alone.
- `pixel-fuzz-s3-locket`: hero Fuzz frame with locket cue visible.
- `pixel-glitch-s4-repair`: hero Glitch frame with repair marks and protected
  face visible.
- `pixel-crystal-s5-facets`: hero Crystal frame with facet cues visible.
- `pixel-mech-late-hardbody`: hero Mech frame with hard-body silhouette and core
  cue visible.
- `pixel-tank-composition`: Pixel pet in an existing tank/prop/tank-life context.

Required typed artifacts:

- Pixel frame JSON for every review frame.
- Pixel art JSON with promoted role cells, role counts, cue coverage, body
  bounds, face/protected bounds, and checksum.
- Pixel fit JSON using the existing schema `2` geometry evidence.
- Tank composition JSON for tank-context fixtures, recording which prop/tank-life
  cells were rendered near protected pet regions and whether any were skipped or
  clipped for pet readability.

The species matrix must not repeat the previous mistake of being mostly a roster
label. It must contain rendered Pixel pet frames for the species/stage cases it
claims to cover.

## Tank Composition Rules

This slice uses existing tank props and ambient tank life. It adds composition
evidence and guardrails only.

Rules:

1. The pet face protected region must not be covered by tank life, foreground
   props, aura, or pulse effects.
2. Signature cues used for hero identity gates must not be covered in tank
   context.
3. Tank life can pass in front of generic body regions, but not through eyes,
   mouth, locket/facet/repair marks, or the HUD-safe text area.
4. Props and tank life should continue to use their current layer semantics.
   If a conflict exists, projection skips or simplifies the prop/tank-life
   element instead of moving the pet into a bad read.
5. The round companion remains a free-float tank. This slice does not add a
   floor, substrate, or new tank mechanics.

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
   collision/occlusion evidence for protected regions.
5. **Privacy:** Pixel art, frame, fit, and tank-composition artifacts pass the
   existing privacy scan pattern, including `.txt`, `.cells.json`, and typed JSON
   files.
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
  typed artifacts, tank-context evidence, and privacy-safe outputs.
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
