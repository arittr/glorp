# Glorp Pixel Default-Readiness Polish - design

- Date: 2026-07-08
- Status: direction approved by Drew; written for review before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-07-08-glorp-smooth-pixel-companion-design.md`
  - `docs/superpowers/measurements/2026-07-08-glorp-smooth-pixel-companion-review.md`
  - `docs/superpowers/specs/2026-07-07-glorp-companion-perimeter-gauges-design.md`

## Problem

The first Smooth Pixel Companion diff proved that Pixel can render a smooth,
portable, opt-in companion pet inside the AppKit round companion. It also left
the right defaults in place: Classic remains the default renderer and Pixel is
explicitly selected with `--renderer pixel`.

The review screenshot and measurement doc show the next blocker. Pixel is alive,
but not default-ready:

- the creature can over-own the aperture and collide visually with the HUD stats
- the pet body reads as a generic procedural block creature, not enough like the
  existing Glorp cast
- minimum-size, resized-window, fullscreen, resize stale-frame, and active CPU
  behavior are not yet reviewed
- the previous Preview Lab contract proves frames exist and animate, but does
  not yet prove HUD-safe fit or alignment with the real pet-art vocabulary

This slice should move Pixel from "animated proof" to "default-readiness
candidate" without flipping the default.

## Direction

Add a Pixel default-readiness polish pass that keeps Pixel as a native smooth
pixel renderer while making it answer to the existing pet art.

The intended art model is hybrid:

1. The existing pet render/art grammar remains the source of truth for species,
   stage growth, silhouette, face placement, and species-specific accents.
2. Pixel derives a sanitized `PixelPetArtReference` from that grammar.
3. The Pixel renderer consumes that reference to draw Pixel-native geometry,
   motion, aura, shadow, and pulse effects.
4. Pixel does not literally blit terminal glyphs or terminal `pet_art` into the
   AppKit companion.

The result should read as "my real Glorp, now alive in a smooth companion," not
as a disconnected mascot and not as a scaled terminal screenshot.

## Goals

1. **Correct-pet alignment.** Pixel frames for hero fixtures visibly follow the
   same species/stage cast as the existing terminal pet art.
2. **HUD-safe fit.** At default companion size, the high-alpha pet body avoids
   the primary HUD stat zones and keeps token/vday text readable.
3. **Portable art-reference seam.** Art reference extraction and Pixel rendering
   remain AppKit-free and testable in Rust unit/integration tests.
4. **Deterministic review contract.** Preview Lab artifacts expose enough
   metadata to review pet bounds, HUD overlap, art-reference identity, animation,
   and privacy without launching AppKit.
5. **Readiness evidence.** The implementation produces review evidence for
   default size, minimum size, resized window, fullscreen or equivalent geometry,
   resize stale-frame behavior, and active/feed-pulse CPU.
6. **Conservative rollout.** Pixel remains opt-in after this slice until the
   review evidence supports a separate default-flip decision.

## Non-goals

- No default renderer flip in this slice.
- No removal of the Classic companion renderer.
- No Linux companion host.
- No literal terminal-art bitmap blit into the companion.
- No external sprite-sheet or asset-pipeline requirement.
- No full rewrite of the existing pet art system.
- No attempt to make every species/stage final mascot art in one pass.

## Art Reference Model

Introduce a small, sanitized art-reference model under the portable
presentation/pixel boundary. The exact module name can be chosen in the
implementation plan, but the conceptual type is:

```rust
PixelPetArtReference {
    species: Species,
    stage: Stage,
    mood: Mood,
    width_cells: u8,
    height_cells: u8,
    occupied_cells: Vec<PixelArtCell>,
    face: Option<PixelFaceReference>,
    body_bounds: PixelCellBounds,
    reference_checksum: PixelReferenceChecksum,
}

PixelArtCell {
    x: u8,
    y: u8,
    role: PixelArtRole,
}

PixelArtRole {
    Body,
    Face,
    Accent,
    Shadow,
    Particle,
}
```

The reference is derived from the same inputs and grammar as the terminal pet:

- `WatchViewModel.pet_render.seed`
- `WatchViewModel.pet_render.generated_species`
- `WatchViewModel.pet_render.stage`
- `WatchViewModel.pet_render.mood`
- current animation beat, sleep state, and feed/activity pulse state
- existing pet palette roles

Implementation may call the existing pet renderer or a shared extraction helper
to produce a reference. The important boundary is that Pixel should consume the
normalized reference, not raw terminal strings as its long-term rendering API.

The reference may use terminal art as an oracle, but it should not export raw
terminal lines to Preview Lab contracts unless there is an explicit review
artifact for human comparison. Machine-readable Pixel artifacts should expose
only sanitized roles, bounds, checksums, and aggregate counts.

## Privacy Contract

Pixel artifacts must keep the privacy posture from the first Pixel diff.

Preview JSON and strip JSON must not include raw seed, source names, exact usage
counts, file paths, project names, diagnostics, prompt text, response text,
transcript text, or user paths.

`reference_checksum` must be stable enough for deterministic review but not a
raw pet seed or reversible seed encoding. It can be derived from sanitized art
roles plus non-secret identity fields, or from a one-way projection over the
same material.

## Pixel Rendering Changes

The current procedural Pixel body should become a fallback grammar, not the only
shape. For the hero fixtures, Pixel should use `PixelPetArtReference` to drive:

- body silhouette proportions and occupied mass
- face/eye anchor placement
- species-specific accent placement
- stage growth scale and complexity
- body-safe blink and asleep poses

Pixel remains Pixel-native:

- raster output is still a logical RGBA `PixelFrame`
- motion remains smooth and time-based
- aura, shadow, pulse, breathing, and wander remain renderer concerns
- AppKit only scales and blits the final frame

The first implementation should prioritize two hero cases:

1. **Fuzz S3 content idle.** Round/fluffy Hearthfloof-like silhouette, readable
   face layer, warm aura, breathing, and blink.
2. **Glitch S4 content feed pulse.** Packet-daemon-like silhouette, bounded
   corruption/accent behavior, readable face, and decaying active pulse.

All other species/stages must remain nonempty, bounded, aperture-safe, and
deterministic, but they may use compatibility mapping from the shared art
reference until later art polish.

## Companion Fit Policy

Pixel should no longer draw the pet frame into the entire circular aperture as a
single undifferentiated square. Add a portable or shared fit policy that defines:

- a creature zone inside the round aperture
- a reserved HUD-safe zone for the central token/vday text
- minimum margins from the circular aperture edge
- body alpha threshold used for collision checks
- scale and vertical placement rules for default, minimum, resized, and large
  bounds

The policy should keep the pet crisp and present while preventing the screenshot
failure mode where the pet body sits behind the primary number.

The fit policy can be enforced in two layers:

1. Pixel frame composition keeps high-alpha body pixels inside a logical
   creature-safe region.
2. AppKit draw placement maps the logical frame into the aperture using the same
   safe-region assumptions.

If text positions are owned by the round HUD renderer, Pixel should use a
shared HUD-safe-zone helper or a test-only approximation derived from the same
round HUD constants. The implementation should avoid duplicating magic numbers
without a named policy.

## Preview Lab Contract

Extend the `pixel` preview scenario with default-readiness artifacts:

- current Pixel frame
- art-reference summary artifact
- content bounds and high-alpha body bounds
- HUD-safe-zone overlap count
- aperture margin status
- renderer mode
- reference checksum
- privacy-scan coverage

Add review fixtures for:

- Fuzz S3 content idle
- Glitch S4 content feed pulse
- a compact/default-size fit fixture
- a minimum-size fit fixture
- a large/fullscreen geometry fixture
- all-species/stage smoke coverage, either as a matrix artifact or tests

Where useful for human review, Preview Lab may show a side-by-side visual:
terminal reference on one side and Pixel-native frame on the other. That visual
artifact is for review only; the Pixel runtime API should stay normalized and
portable.

## AppKit Review Contract

The implementation should close or explicitly reclassify the review deferrals in
`docs/superpowers/measurements/2026-07-08-glorp-smooth-pixel-companion-review.md`.

Required evidence:

- default-size Pixel screenshot
- minimum-size Pixel screenshot or deterministic geometry artifact
- resized-window Pixel screenshot or deterministic geometry artifact
- fullscreen Pixel screenshot or deterministic geometry artifact
- resize stale-frame behavior evidence
- idle CPU comparison against Classic
- active/feed-pulse CPU comparison against Classic

If AX/AppleScript still cannot mutate the borderless companion window, the
implementation should add a hidden deterministic review path rather than making
the readiness claim depend on fragile desktop automation. Acceptable examples:

- a hidden companion geometry command that renders AppKit-equivalent placement
  metadata for selected bounds
- Preview Lab fixtures that use the same fit policy and HUD-safe helper as the
  runtime
- a runtime debug option that launches the companion at a requested initial size
  without changing normal user behavior

The plan should pick the smallest reliable path after grounding the current
AppKit constraints.

## Active CPU Review

The previous review could only measure idle CPU because no deterministic live
usage pulse was available. This slice should add a deterministic active-pulse
review path.

The review path should exercise the same Pixel feed/activity pulse rendering
path used by live companion state. It should not require waiting for real user
token usage. The implementation may use a test fixture, a hidden dev command, or
a local usage-store seed script, provided the evidence clearly states which path
was used and why it matches the live rendering path.

## Testing

Automated coverage should include:

- `PixelPetArtReference` is deterministic for the same identity/animation input
- different hero species/stages produce meaningfully different references
- Pixel frames are nonempty for all species/stages
- hero Pixel frames differ from the old generic fallback enough to prove the
  reference is used
- high-alpha Pixel body bounds stay out of the HUD-safe zone for target
  geometries
- feed-pulse frame changes remain bounded and decay over time
- Preview Lab manifests include the new Pixel default-readiness metadata
- Preview Lab privacy scan covers art-reference metadata
- no AppKit dependency enters portable pixel modules

Manual or measurement coverage should include:

- Preview Lab render of the default-readiness fixtures
- AppKit launch of Classic/default to prove default behavior is unchanged
- AppKit launch of Pixel through `--renderer pixel`
- screenshot or geometry evidence for default/min/resized/fullscreen bounds
- active and idle CPU sampling with commands recorded in the measurement doc

The expected automated gate is at least:

```bash
cargo fmt --check
cargo test
cargo test --features dev-preview --test dev_preview
cargo test --features dev-preview dev_preview::scenarios
cargo test --features dev-preview dev_preview::export
cargo clippy --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features --all-targets
```

## Acceptance Criteria

This slice is complete when:

1. Pixel remains opt-in and Classic remains the default companion renderer.
2. Fuzz S3 idle and Glitch S4 feed-pulse Pixel fixtures visibly track the real
   pet-art cast instead of the generic procedural body.
3. Pixel high-alpha body bounds do not overlap the primary HUD-safe zone in the
   target default-readiness fixtures.
4. Preview Lab exports deterministic art-reference, bounds, fit, and privacy
   metadata.
5. AppKit or AppKit-equivalent geometry evidence covers default, minimum,
   resized, and fullscreen target bounds.
6. Active/feed-pulse CPU evidence exists and is compared against Classic.
7. The measurement doc states whether Pixel is ready for a separate default-flip
   decision. This slice should not perform that flip.

## Default Flip Decision

The default flip is a later, separate decision. This slice should produce the
evidence needed for that decision and leave the runtime default unchanged.

If the evidence is strong, the follow-up can be a small default-flip diff with a
clear rollback path. If the evidence exposes remaining art, fit, CPU, or AppKit
issues, those become explicit follow-up blockers rather than hidden risk.
