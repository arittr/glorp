# Glorp macOS round companion - design

- Date: 2026-06-13
- Status: direction approved by Drew; written for review before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-05-12-glorp-preview-lab-design.md`
  - `docs/superpowers/specs/2026-05-13-watch-component-system-design.md`
  - `docs/superpowers/specs/2026-06-11-glorp-alive-room-design.md`
  - `docs/superpowers/specs/2026-06-11-glorp-activity-identity-design.md`
  - `docs/superpowers/specs/2026-06-13-glorp-species-room-dialects-design.md`

## Problem

`glorp watch` is a good terminal dashboard, but it is still a terminal
dashboard. The Napster View reference is interesting because it is a small
always-visible presence surface on an external display, not because Glorp
should copy its AI-specialist product or hardware.

Drew wants the same kind of glanceable external-screen feeling for Glorp: the
pet should be visible as a calm companion while work happens elsewhere. A
terminal pretending to be round is useful for debugging and deterministic
review, but it does not make sense as the user-facing feature.

## Direction

Build a native macOS companion window as the first real facade for the round
view.

The product surface is:

```text
Glorp companion for macOS
  -> launched like a normal app
  -> owns its Dock/app lifecycle
  -> opens a polished round companion window
  -> keeps using Glorp's local state, ledger, and usage polling
```

The developer surfaces are:

```text
glorp dev-preview --scenario round   # deterministic review artifacts
glorp watch --view round             # optional hidden/debug harness only
```

The round view is a porthole into the pet's room. It is not a shrunken watch
screen.

## Goals

- Create a native macOS companion surface suitable for a normal external
  display or small always-visible window.
- Make the pet and room the whole experience: cute, alive, calm, and readable
  at a glance.
- Keep `glorp watch` as the terminal TUI and keep round terminal output as
  debug-only, if it exists at all.
- Reuse the existing local data model: `WatchViewModel`, `DayContext`,
  `PetLifeProfile`, `ActivityIdentityProfile`, habitat props, source health,
  and pet render state.
- Add a renderer-neutral `RoundSceneModel` boundary so later hardware
  framebuffers or browser/canvas renderers can consume the same semantic scene.
- Preserve the existing privacy boundary: no prompts, responses, tool payloads,
  source files, project names, file paths, transcripts, quotas, or productivity
  pressure.
- Prove the design in Preview Lab before relying on live native rendering.

## Non-goals

- No V1 hardware integration or device protocol work.
- No V1 framebuffer, HID, USB-C display, or vendor SDK support.
- No V1 full dashboard in the companion window.
- No exact token counts, feed rows, timestamps, rate-per-hour labels, cost,
  ETAs, streaks, leaderboards, or work score in the round view.
- No rewrite of the watch TUI.
- No new persisted pet-state schema unless implementation proves a tiny
  window-placement preference is necessary.
- No publishing or packaging redesign in this spec. App bundling details can be
  planned after the product and renderer boundaries are approved.

## Product Model

The companion answers one question:

> Is my Glorp here, alive, okay, and reacting to real work?

The full watch remains the place for accounting and debugging:

- source breakdowns
- feed rows
- helper diagnostics
- progress numbers
- bio details
- keyboard controls

The round companion keeps only ambient signals:

- pet pose, blink, breath, asleep/awake state
- room biome and earned-prop identity
- day phase and calm/weather texture
- species dialect, especially Glitch vs Crystal distinctiveness
- recent activity pulse
- source diversity as small color/shape accents
- vitals or trouble state as tiny rim indicators
- one clear degraded/helper-blocked signal

## Visual Design

Use a "Glorp Porthole" composition:

```text
outer halo / status beads
  circular room aperture
    upper-air texture
    pet centered as emotional anchor
    one or two earned props as landmarks
    lower floor crescent
```

The round view should feel like looking into the pet's little room, not reading
a watch face.

### Halo Signals

The halo uses segments or beads instead of text labels.

- Recent activity: faint sweep or short pulse that appears only after real
  applied usage.
- Source diversity: one bead for a single-source day, two beads for balanced
  use, a small cluster for ensemble use.
- Vitals: three tiny lower beads whose intensity/color reflects fed,
  happiness, and energy.
- Helper trouble: a small warning bead at the rim.
- Stage/progress: subtle aura, crown, or growth accent. Do not print XP.

These signals must remain optional and quiet. The pet and room carry the
surface; the halo only helps at a glance.

### Room Compression

The companion should reuse Alive Room and Species Room Dialect semantics, but
compress them for the aperture:

- The room chooses one dominant biome silhouette.
- The renderer keeps at most one or two visually important prop landmarks.
- Background texture uses day phase, work weather, and species dialect.
- The pet never clips the safe inner circle.
- If the window is very small, prop detail drops before pet legibility drops.

### Motion

Motion should be semantic and calm:

- pet breath/blink
- slow one-cell wander or equivalent native motion
- real feed pulse sweep
- one prop resonance ping/ripple
- soft dawn/night transition
- asleep mode with dimmed room and nearly still halo

Motion must not imply fake work. Clock-only ambience may make the room feel
alive, but activity pulses must come from real usage signals.

## Architecture

Introduce a small semantic scene boundary:

```rust
pub struct RoundSceneModel {
    pub pet: RoundPetModel,
    pub room: RoundRoomModel,
    pub halo: RoundHaloModel,
    pub lifecycle: RoundLifecycleModel,
}
```

Exact names can change, but the ownership should not:

```text
WatchViewModel
  -> derive_round_scene_model(vm, now)
  -> renderer-specific output
```

V1 renderer targets:

- Preview Lab renderer for deterministic artifacts.
- Native macOS companion renderer for the real user-facing surface.

Future renderer targets:

- browser/canvas local companion
- hardware framebuffer
- direct device protocol output

The scene model must not contain AppKit types, terminal cell coordinates, or
hardware-specific fields. It contains semantic choices: pet state, room biome,
prop landmark choices, halo signal states, and animation seeds.

### Native macOS App

The user-facing companion should behave like a normal macOS app:

- It has Dock/app lifecycle rather than occupying a terminal session.
- The window can be placed on an external display.
- The terminal command, if present, launches or opens the app and exits.
- The app continues polling and animating after launch.
- Closing the companion window does not imply terminal shutdown because there
  is no terminal owner. V1 should keep the app running in the Dock until the
  user quits the app, and reopening from the Dock should restore the companion
  window.
- The V1 renderer should be native AppKit/Core Animation/Core Graphics code
  behind an `NSView`-style surface. A WebView/canvas renderer is a possible
  future target, not the first macOS facade.

The existing `glorp menubar` code is useful precedent because it already uses
the same watch polling and view model inside AppKit. It should not define the
product UX. Menubar remains a debug/internal-ish facade until separately
designed.

### Command Surface

Preferred product surface:

```text
Glorp.app
```

Possible CLI launcher:

```bash
glorp companion
```

The launcher starts or opens the native app and exits. It does not run the
facade inline.

Debug/developer surfaces:

```bash
glorp dev-preview --scenario round
glorp watch --view round
```

`glorp watch --view round` is optional and should be hidden or clearly
developer-only. It exists only if it materially helps debug the scene model or
renderer.

## Data Flow

The companion uses the same source of truth as watch:

```text
state.json + usage.sqlite
  -> build_watch_view_model
  -> live usage poll/apply loop
  -> WatchViewModel
  -> RoundSceneModel
  -> macOS renderer
```

No new ingestion path is introduced. No source identity or cursor logic moves
into the companion.

The native renderer may own transient animation state, but semantic scene
selection remains derived from the view model.

## Error Handling

If no pet exists, the launcher/native app should present a simple native
message directing the user to initialize Glorp. The exact UI can be planned
later; it must not panic or silently open an empty window.

If usage helpers are blocked or degraded, the companion shows one small trouble
bead/rim signal. Details remain in `glorp watch`, `glorp status`, or
`glorp doctor`.

If polling fails transiently, keep the last good scene and show a subtle stale
or trouble signal only after the existing watch model would consider the source
blocked/degraded. Do not create a new failure taxonomy in the companion.

If the display or window is too small, degrade in this order:

1. hide optional halo detail
2. reduce prop landmarks
3. simplify room texture
4. keep pet legible
5. show a minimal native empty/error state if pet legibility is impossible

## Preview Lab

Add a round scenario before implementing or trusting the live native facade.

Useful fixtures:

- `round-normal`
- `round-active-pulse`
- `round-asleep-night`
- `round-helper-trouble`
- `round-flat-color`
- `round-glitch-dialect`
- `round-crystal-dialect`

Artifacts should include manifest entries with:

- dimensions
- scene model inputs
- target renderer
- color capability
- privacy notes
- review prompts

Acceptance checks:

- pet is inside the safe inner circle
- corners outside the aperture are intentionally blank/transparent in preview
- no long labels or dashboard rows appear
- flat-color view is still readable by glyph/shape
- Glitch and Crystal differ by non-color texture
- helper trouble is visible without text
- activity pulse exists only for real applied usage fixture inputs
- animation strips keep fixed bounds and do not clip the pet

## Testing

Unit tests should cover `derive_round_scene_model` as a pure transformation
from `WatchViewModel` plus time.

Preview tests should cover fixture presence, manifest metadata, and basic
geometry/mask invariants.

Native app tests can stay lighter in V1 because AppKit UI testing is expensive.
The implementation plan should still include a local smoke path for launching
the companion on macOS and confirming it can build a scene from real local
state.

## Open Implementation Questions

These do not block the design direction, but they should be resolved during
implementation planning:

- Whether the app bundle is produced by the npm package, a macOS-only helper,
  or a separate release artifact.
- Whether `glorp companion` can open an installed app bundle or should run a
  native facade in-process during development only.
- Whether window placement/size should persist in user defaults or Glorp config.
- Whether V1 needs any menu-bar affordance in addition to the required Dock
  lifecycle.

## Recommendation

Spec and build V1 as native macOS companion first:

1. Define `RoundSceneModel`.
2. Add Preview Lab round scenarios and acceptance checks.
3. Add a native macOS companion facade that launches like an app and uses the
   same state/polling model as watch.
4. Keep terminal round output hidden/debug-only, or skip it unless it helps
   inspect the scene model.

This keeps the product honest: the round feature is a polished companion
window, not a terminal trick. It also keeps future hardware plausible by
putting the reusable boundary at the semantic scene model instead of inside
AppKit.
