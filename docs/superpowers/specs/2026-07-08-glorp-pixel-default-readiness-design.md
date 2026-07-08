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
5. **Readiness gates.** The implementation produces pass/fail evidence for
   default size, minimum size, resized window, fullscreen or equivalent geometry,
   resize stale-frame behavior, HUD readability, and active/feed-pulse CPU. Mere
   existence of evidence is not enough; failing evidence blocks a default flip.
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
presentation/pixel boundary. The implementation plan should name the exact
modules, but the intended boundary is:

- art extraction lives in `pet` or a presentation module that can call `pet`
- Pixel scene and frame rendering stays under `presentation::pixel`
- fit/HUD-safe geometry lives in a pure shared helper, likely under `round`
- AppKit code in `companion` only uses those helpers and draws the final result

The reference is produced by a `PixelPetArtReferenceProvider` or equivalent
helper with an explicit key and cache policy. It must not rebuild terminal
strings on every 30fps Pixel tick. Recompute only when the discrete art pose key
changes: species, stage, mood, palette/variation projection, sleep/blink/face
pose, work accent, feed reaction, or Glitch repair/corruption state. Continuous
Pixel-only motion such as wander, breath, aura, shadow, and pulse interpolation
must not invalidate the art reference.

The conceptual type is:

```rust
PixelPetArtReference {
    species: Species,
    stage: Stage,
    mood: Mood,
    pose: PixelArtPoseKey,
    width_cells: u8,
    height_cells: u8,
    occupied_cells: Vec<PixelArtCell>,
    face: Option<PixelFaceReference>,
    body_bounds: PixelCellBounds,
    foot_contact: PixelFootContact,
    reference_checksum: PixelReferenceChecksum,
}

PixelArtCell {
    x: u8,
    y: u8,
    role: PixelArtRole,
}

PixelArtRole {
    Body,
    BodyGlow,
    Eye,
    Mouth,
    Accent,
    Pattern,
    Particle,
    Corruption,
    Outline,
    InteriorTexture,
    Appendage,
    FootContact,
    Locket,
    Facet,
    RepairMark,
}
```

The reference is derived from the same inputs and grammar as the terminal pet:

- a projected variation key derived from `WatchViewModel.pet_render.seed`; the
  reference must never store the raw seed
- `WatchViewModel.pet_render.generated_species`
- `WatchViewModel.pet_render.stage`
- `WatchViewModel.pet_render.mood`
- current animation beat, sleep state, and feed/activity pulse state
- existing pet palette roles

The extraction source must be canonical rendered pet output:
`RenderedPet { lines, spans }` from the existing pet renderer, or a shared helper
that returns the same rendered/spanned result. Template-only extraction is not
enough because it can miss mood eyes, mouth, sleep/blink state, feed reaction,
work accent, Glitch repair marks, and transient corruption.

Animation ownership has two layers:

1. The art reference pose is discrete. It owns face, eye, mouth, blink/asleep
   pose, Glitch repair/corruption roles, and other role cells derived from the
   canonical terminal art beat.
2. The Pixel scene is continuous. It owns subpixel wander, breathing, aura,
   shadow, pulse interpolation, and scale/placement inside the companion.

The same discrete pose key must produce the same `PixelPetArtReference`
independent of the 30fps Pixel elapsed time.

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

Machine-readable art-reference JSON must use an allowlist, not only a blacklist.
Allowed fields are sanitized identity labels, dimensions, role cells, aggregate
role counts, bounds, pose labels, fit metrics, and one-way checksums derived from
sanitized role cells. It must not include raw terminal `art_text`, raw seed,
source labels, exact usage counts, paths, diagnostics, prompt/response text, or
transcripts.

Human review may include a separate side-by-side terminal-art reference artifact.
That artifact is not the Pixel runtime API and should be privacy-scanned
separately from the machine-readable Pixel reference contracts.

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

The cutover rule is strict: `PixelPetScene` should derive silhouette, face
anchors, and accent masks from `PixelPetArtReference` wherever a reference is
available. The old species-match procedural body can remain only as an explicit
fallback path for missing or invalid references. Acceptance tests should prove
hero frames change when the art reference changes, not merely when `Species`
changes.

The first implementation should prioritize two hero cases:

1. **Fuzz S3 content idle.** Round/fluffy Hearthfloof-like silhouette, readable
   face layer, warm aura, breathing, and blink.
2. **Glitch S4 content feed pulse.** Packet-daemon-like silhouette, bounded
   corruption/accent behavior, readable face, and decaying active pulse.

Hero oracle requirements:

| Fixture | Source-art cues Pixel must preserve | Machine checks |
| --- | --- | --- |
| Fuzz S3 content idle | twin ear-cones, rounded body, face rows, locket marker, mitten-foot/contact cells | occupied count within the source stage band, face inside body, locket/foot-contact roles present, bounds match the 11x8 art core after normalization |
| Glitch S4 content feed pulse | leaning wafer/packet silhouette, alive lens face, belly/body repair-mark candidates, transient corruption during feed reaction, foot/contact row | Glitch S4 occupied count and face-slot invariants reused, `Eye`/`Mouth` protected, `RepairMark` and bounded `Corruption` roles represented distinctly, feed-pulse corruption decays |

All other species/stages must remain nonempty, bounded, aperture-safe, and
deterministic, but smoke coverage alone is not enough. Each species must have a
minimum identity cue in the Pixel reference and at least one side-by-side Preview
Lab review fixture:

| Species | Minimum Pixel identity cue |
| --- | --- |
| Fuzz | ear-cones, locket/body marker, mitten or foot-contact cells |
| Blob | bell/core silhouette and tendril/contact cells |
| Ghost | shroud body, living face, and lower hem/contact cue |
| Glitch | packet/wafer silhouette, repair/corruption roles, protected living face |
| Crystal | facet roles and lens/face placement |
| Mech | chassis outline, sensor/face placement, mechanical pattern/accent cells |

Stage coverage should cover at least juvenile, mid, and elder bands per species,
with all species x all stages covered by deterministic bounds/role smoke tests.

## Companion Fit Policy

Pixel should no longer draw the pet frame into the entire circular aperture as a
single undifferentiated square. Add an authoritative production fit policy, such
as `PixelCompanionFit`, that defines:

- a creature zone inside the round aperture
- a reserved HUD-safe zone for the central token/vday text
- minimum margins from the circular aperture edge
- body alpha threshold used for collision checks
- scale and vertical placement rules for default, minimum, resized, and large
  bounds

The policy should keep the pet crisp and present while preventing the screenshot
failure mode where the pet body sits behind the primary number.

The fit policy must be enforced in two layers:

1. Pixel frame composition keeps high-alpha body pixels inside a logical
   creature-safe region.
2. AppKit draw placement maps the logical frame into the aperture by calling the
   same production placement helper used by Preview Lab and tests.

If text positions are owned by the round HUD renderer, Pixel should use a
shared HUD-safe-zone helper derived from the same layout inputs as AppKit HUD
drawing: aperture, gauge gap, stat gap, font scaling, line spacing, and worst
case HUD text. A test-only approximation may be useful during development, but
it is not authoritative and cannot satisfy the readiness gate.

Target geometries:

- default: `360x360`
- minimum: `260x260`
- resized: one representative larger square, at least `480x480`
- fullscreen-equivalent: the largest square that fits the current display work
  area, or a deterministic Preview Lab geometry using the same production helper

Readability checks should exercise worst-case HUD strings, including large token
totals, large positive/negative yesterday percentages, and burst pace text.
Body, eye, and mouth pixels must have zero overlap with the HUD-safe text zone.
Translucent aura/accent/pulse overlap may be allowed only when bounded by the
fit policy and when text contrast remains readable.

## Preview Lab Contract

Extend the `pixel` preview scenario with default-readiness artifacts:

- current Pixel frame
- sanitized art-reference sidecar artifact, for example
  `frames/<id>.pixel-art.json` with its own schema version
- fit/readability sidecar artifact, for example `frames/<id>.pixel-fit.json`
  with its own schema version
- content bounds, role bounds, and high-alpha body/face bounds
- HUD-safe-zone overlap count by role and alpha band
- aperture margin status from the production fit helper
- renderer mode
- reference checksum
- privacy-scan coverage

The existing `.pixel.json` frame schema may remain focused on RGBA pixels. New
metadata should live in typed sidecars unless the implementation deliberately
bumps `PIXEL_FRAME_SCHEMA_VERSION`. Either path must bump the manifest contract
where needed and add tests that assert the new files are listed.

Add review fixtures for:

- Fuzz S3 content idle
- Glitch S4 content feed pulse
- a compact/default-size fit fixture
- a minimum-size fit fixture
- a large/fullscreen geometry fixture
- all-species/stage smoke coverage, either as a matrix artifact or tests

Preview Lab must show a side-by-side human visual for the hero fixtures:
canonical terminal-art reference on one side and Pixel-native frame on the
other. At least one side-by-side fixture per species should be available for the
minimum cast-identity matrix. The side-by-side visual is for review only; the
Pixel runtime API stays normalized and portable.

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

- a hidden companion geometry command that renders placement metadata by calling
  the same production fit helper as AppKit drawing
- Preview Lab fixtures that use the same fit policy and HUD-safe helper as the
  runtime
- a runtime debug option that launches the companion at a requested initial size
  without changing normal user behavior

Preview-only geometry is not sufficient unless it calls the exact production
placement helper that `draw_pixel_frame` uses. The plan should pick the smallest
reliable path after grounding the current AppKit constraints.

Resize stale-frame behavior must be defined and measured: after a resize, Pixel
must recompute placement by the next render tick, with no stretched old frame
remaining visible after 100ms.

## Active CPU Review

The previous review could only measure idle CPU because no deterministic live
usage pulse was available. This slice should add a deterministic active-pulse
review path.

The review path should exercise the same Pixel feed/activity pulse rendering
path used by live companion state. It should not require waiting for real user
token usage. The implementation may use a test fixture, a hidden dev command, or
a local usage-store seed script, provided the evidence clearly states which path
was used and why it matches the live rendering path.

Preview-only pulse fixtures are not enough for CPU readiness. The active review
must inject or seed state through the live companion presentation path that
stamps feed/activity pulses from an applied usage signal, or through a hidden dev
entry point that calls the same state transition.

Measurement protocol:

1. Use the same build mode, window size, and machine for Classic and Pixel.
2. Warm each process for at least 10 seconds before sampling.
3. Record `top -pid "$pid" -stats pid,command,cpu,time -l 12 -s 5` for each
   mode and preserve raw samples.
4. Record one `sample "$pid" 10 -file <path>` for idle and one for active Pixel
   to attribute stack cost.
5. Report average CPU excluding the first `top` sample, p95 CPU over the kept
   samples, and any obvious object-allocation or rendering hot spots.

Pass budget for a default-flip candidate:

- Pixel idle average CPU must be no more than 5 percentage points above Classic
  idle average on the same run, and Pixel idle p95 must be no more than 10
  percentage points above Classic idle p95.
- Pixel active average CPU must be no more than 5 percentage points above
  Classic active average on the same run, and Pixel active p95 must be no more
  than 10 percentage points above Classic active p95.
- A failing CPU budget does not necessarily fail this implementation slice, but
  it blocks the measurement doc from recommending a default flip.

The measurement doc should record raw values, command lines, PIDs, build mode,
window size, and whether each CPU gate passed, failed, or was blocked.

## Default-Readiness Gates

This slice should end with a pass/fail gate table in the measurement doc.

| Gate | Required artifact | Pass threshold | Failure behavior |
| --- | --- | --- | --- |
| Runtime fit authority | test/Preview artifact names the production fit helper also used by AppKit draw | Preview and AppKit placement use the same helper; no test-only approximation can satisfy readiness | blocks default flip |
| HUD body overlap | fit artifacts for `360x360`, `260x260`, resized, and fullscreen-equivalent geometries | zero body/eye/mouth overlap with HUD text zone; bounded translucent effect overlap with readable text contrast | blocks default flip |
| Cast identity | hero side-by-side artifacts and species matrix | hero oracles pass; every species has minimum identity cue and non-generic representative fixture | blocks default flip |
| All species/stages smoke | automated cross-product test | every species x stage is nonempty, bounded, deterministic, aperture-safe, and uses reference roles where available | blocks default flip |
| Active pulse path | live-path or equivalent hidden-dev injection evidence | active pulse reaches Pixel through same state transition as companion live updates | blocks default flip |
| CPU budget | top/sample artifacts for Classic/Pixel idle and active | Pixel stays within the average and p95 overhead budgets above | blocks default flip |
| Resize freshness | screenshot or runtime geometry/redraw artifact | placement recomputes by next render tick; no stretched stale frame after 100ms | blocks default flip |
| Privacy | allowlist tests and artifact scan | no raw seed, raw terminal art in machine JSON, source labels, exact usage counts, paths, diagnostics, prompts, responses, or transcripts | blocks merge until fixed |

## Testing

Automated coverage should include:

- `PixelPetArtReference` is deterministic for the same identity/animation input
- `PixelPetArtReference` is unchanged across continuous 30fps elapsed times when
  the discrete pose key is unchanged
- art-reference extraction is cached or keyed so terminal rendered art is not
  rebuilt on every smooth Pixel tick
- different hero species/stages produce meaningfully different references
- Pixel frames are nonempty for all species/stages
- every species x every stage is nonempty, bounded, aperture-safe, and
  deterministic
- hero Pixel frames respond to changes in `PixelPetArtReference`, not just
  changes in `Species`
- Fuzz S3 and Glitch S4 tests reuse existing art/render invariants for stage
  bands, occupied counts, face slots, role spans, and Glitch repair/corruption
  behavior
- high-alpha Pixel body bounds stay out of the HUD-safe zone for target
  geometries
- feed-pulse frame changes remain bounded and decay over time
- Preview Lab manifests include the new Pixel default-readiness metadata
- Preview Lab privacy scan covers art-reference metadata with a strict JSON
  allowlist
- no AppKit dependency enters portable pixel modules
- `companion` and `companion-app` CLI defaults remain Classic

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
   pet-art cast according to their hero oracle checks, not merely according to
   generic pixel-count deltas.
3. Every species has at least one Pixel side-by-side review fixture with a
   minimum identity cue, and every species x stage passes deterministic
   nonempty/bounds/reference-role smoke tests.
4. Pixel body, eye, and mouth bounds do not overlap the HUD-safe text zone in
   the target default-readiness fixtures; translucent effects remain bounded and
   text contrast stays readable.
5. Preview Lab exports deterministic art-reference, bounds, fit, and privacy
   metadata through typed sidecar artifacts or an explicit frame schema bump.
6. AppKit or AppKit-equivalent geometry evidence covers default, minimum,
   resized, and fullscreen target bounds using the same production fit helper as
   runtime drawing.
7. Active/feed-pulse CPU evidence uses the live companion state transition or an
   equivalent hidden-dev path, reports the measurement protocol, and compares
   Classic/Pixel average and p95 CPU against the stated budgets.
8. The measurement doc contains the default-readiness gate table and states
   whether Pixel is ready for a separate default-flip
   decision. This slice should not perform that flip.

## Default Flip Decision

The default flip is a later, separate decision. This slice should produce the
evidence needed for that decision and leave the runtime default unchanged.

If the evidence is strong, the follow-up can be a small default-flip diff with a
clear rollback path. If the evidence exposes remaining art, fit, CPU, or AppKit
issues, those become explicit follow-up blockers rather than hidden risk.
