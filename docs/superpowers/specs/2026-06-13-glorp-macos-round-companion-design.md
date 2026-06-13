# Glorp macOS round companion - design

- Date: 2026-06-13
- Status: direction approved by Drew; written for review before implementation planning
- Builds on:
  - `docs/superpowers/specs/2026-05-12-glorp-preview-lab-design.md`
  - `docs/superpowers/specs/2026-05-13-watch-component-system-design.md`
  - `docs/superpowers/specs/2026-06-11-glorp-alive-room-design.md`
  - `docs/superpowers/specs/2026-06-11-glorp-activity-identity-design.md`
  - species room dialect direction, currently tracked in the active working tree
    as `docs/superpowers/specs/2026-06-13-glorp-species-room-dialects-design.md`.
    If that spec is not committed before this companion work begins, the
    companion implementation must inline the minimal dialect contract it needs:
    Glitch and Crystal round scenes must differ by non-color glyph/texture
    language while preserving earned prop identity.

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
- No full release-system redesign in this spec. V1 still needs an explicit
  macOS launch/distribution contract before implementation planning; that
  contract should be small and should not attempt to solve future hardware or
  cross-platform app packaging.

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

The round companion keeps only ambient signals. After visual brainstorming, the
approved V1 direction is **Active Halo constrained by Quiet Porthole
restraint**:

- Quiet Porthole is the emotional baseline: pet and room first, almost no UI.
- Active Halo is the V1 default: the same porthole plus one real activity pulse
  and a few tiny rim beads.
- Night Calm is a state variant for asleep/night/calm mode, not the default.

V1 required signals:

- pet pose, blink, breath, asleep/awake state
- room biome and earned-prop identity
- day phase and calm/weather texture
- recent activity pulse
- one clear degraded/helper-blocked signal

V1 optional if cheap:

- species dialect, especially Glitch vs Crystal distinctiveness
- vitals as tiny lower rim beads
- source diversity as abstract color/shape accents

Post-V1:

- richer source-diversity clusters
- multiple simultaneous prop landmarks
- detailed stage/progress aura variants

Because this runs on an external display, source/activity signals default to
abstract presentation. The companion may show that activity happened, but it
must not reveal exact source names, exact counts, project context, or work
cadence in a way that reads like surveillance. A future quiet/private display
toggle can further suppress activity/source beads.

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
    pub moments: Vec<RoundSceneMoment>,
}
```

Exact names can change, but the ownership should not:

```text
WatchViewModel
  -> derive_round_scene_model(vm, now)
  -> layout_round_scene(scene, aperture, capabilities)
  -> renderer-specific output
```

V1 renderer targets:

- Preview Lab renderer for deterministic artifacts.
- Native macOS companion renderer for the real user-facing surface.

Future renderer targets:

- browser/canvas local companion
- hardware framebuffer
- direct device protocol output

The scene model must not contain AppKit types, terminal cell coordinates,
terminal-rendered `pet_art`, or hardware-specific fields. It contains
allowlisted semantic choices only:

- pet seed/species/stage/mood and expression hints
- coarse vitals buckets, not exact percentages
- day phase, asleep/calm state, and work-weather category
- room biome, species dialect key, and selected prop landmark IDs
- source-diversity category, not source names in the companion surface
- helper health category, not diagnostic text
- abstract activity pulse state, not token counts or event rows
- renderer-neutral scene moments

It must not contain:

- `recent_events`
- `errors`
- `helper_status`
- exact token totals, rates, or costs
- source display names for the visible companion
- prompts, responses, command text, file paths, project names, or transcripts

Renderer/capability geometry belongs in a separate layout layer:

```rust
pub struct RoundSceneLayout {
    pub aperture: RoundAperture,
    pub safe_inner_radius: f32,
    pub pet_anchor: RoundAnchor,
    pub prop_anchors: Vec<RoundAnchor>,
    pub halo_anchors: Vec<RoundAnchor>,
    pub motion_budget: RoundMotionBudget,
}
```

Exact names can change. The split should not. Preview, AppKit, and future
hardware renderers should derive layout from the same aperture size, safe
radius, color capability, and motion budget so the "pet never clips" rule is
not reimplemented differently in each renderer.

Scene moments are also renderer-neutral:

```rust
pub struct RoundSceneMoment {
    pub kind: RoundSceneMomentKind,
    pub trigger_id: String,
    pub anchor: RoundMomentAnchor,
    pub duration_ms: u16,
    pub replay_policy: RoundReplayPolicy,
}
```

Renderers map anchors to native views, preview masks, or future framebuffer
regions. The round model must not expose terminal target IDs such as
`watch.pet.effect`.

The pure round model and layout derivation must live outside the `dev-preview`
feature. Only preview export and preview CLI plumbing are feature-gated. Release
builds and the native companion must consume the same pure modules.

### Native macOS App

The user-facing companion should behave like a normal macOS app:

- It has Dock/app lifecycle rather than occupying a terminal session.
- It is a regular app, not `LSUIElement`, and must not use the current
  menubar-only accessory activation policy as-is.
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
- AppKit/window/view ownership stays on the main thread. Background poll work
  sends owned, sendable scene snapshots or stamped view models back to the main
  thread; it never touches AppKit objects.

The existing `glorp menubar` code is useful precedent because it already uses
the same watch polling and view model inside AppKit. It should not define the
product UX. Menubar remains a debug/internal-ish facade until separately
designed.

### V1 Launch And Distribution Contract

Implementation planning must make "launched like a normal app" real before
building renderer polish.

V1 contract:

- Artifact: a macOS `.app` bundle named `Glorp.app`.
- Bundle identity: a new companion/default app identity, not the existing
  menubar-only `dev.glorp.menubar` identity.
- Activation: regular Dock-visible app; no `LSUIElement=true`.
- Version: derived from the same release version surfaces as the Rust/npm
  package.
- Launcher: a CLI helper may exist, but it opens the installed app bundle via
  LaunchServices/`open` and exits. It does not run a long-lived AppKit facade
  inline through the npm `spawnSync` wrapper.
- Repeated launch: focusing/reopening an already-running app restores the
  companion window.
- Unsupported platforms: the companion launcher reports a concise macOS-only
  message and exits without affecting normal CLI commands.
- Release ownership: decide during implementation planning whether the bundle
  ships inside the macOS npm platform package or as a separate macOS artifact.
  Do not start implementation with this unresolved.

Helper discovery is part of this contract. A Dock/Finder-launched app does not
inherit npm wrapper environment variables such as helper binary paths. V1 must
choose one of these before implementation:

- bundle helper binaries/resources inside `Glorp.app` and resolve them relative
  to the bundle;
- write a shared helper-locator config during npm install or first CLI launch
  that the app can read without environment inheritance;
- or make the app launch mediated by an installed helper that still exits
  immediately after registering the required app environment.

The selected path must include a smoke check for a no-env Dock/Finder launch.

### V1 Window Contract

The companion window is part of the product, not an AppKit afterthought.

V1 window behavior:

- default shape is visually round/porthole-like;
- no dashboard labels inside the product surface;
- default size is large enough for the current pet silhouette plus room
  crescent; implementation planning should pick exact pixels after previewing
  the smallest legible aperture;
- minimum size preserves pet legibility before preserving optional halo detail;
- default window level is normal or floating-above-normal only if Drew approves
  that behavior during implementation planning;
- closing the window keeps the app alive in the Dock;
- Dock reopen restores or recreates the companion window;
- placement should persist using macOS user defaults unless implementation
  finds a strong reason to prefer Glorp config;
- if the prior display is missing, restore on the main display without losing
  the saved placement.

### Command Surface

Preferred product surface:

```text
Glorp.app
```

V1 CLI launcher:

```bash
glorp companion
```

On macOS, the launcher starts or opens the installed native app and exits. On
other platforms it reports that the native companion is macOS-only. If an
installed app bundle cannot be found in development, the command may suggest
the local app-build command once that command exists; it should not silently
fall back to an inline terminal facade.

Debug/developer surfaces:

```bash
glorp dev-preview --scenario round
```

Do not add `glorp watch --view round` to V1. If a later implementation needs a
terminal harness, it must be hidden/developer-only and omitted from README/npm
docs.

## Data Flow

The companion uses the same source of truth as watch:

```text
state.json + usage.sqlite
  -> build_watch_view_model
  -> live usage poll/apply loop
  -> LifeSignalState / presentation stamping
  -> stamped WatchViewModel
  -> RoundSceneModel
  -> RoundSceneLayout
  -> macOS renderer
```

No new ingestion path is introduced. No source identity or cursor logic moves
into the companion.

The companion must reuse or extract the existing watch presentation stamping
loop: poll result, applied usage signal, `LifeSignalState`, feed-pulse time,
source accent/work weather, and calm-mode state are applied before deriving
`RoundSceneModel`. Calling `build_watch_view_model` alone is not sufficient for
live companion state.

The native renderer may own transient animation state, but semantic scene
selection remains derived from the stamped view model and round scene model.

Polling contract:

- one in-flight poll per companion process;
- helper subprocess and SQLite work always run off the AppKit main thread;
- the main thread receives owned results or errors and updates native views;
- if another Glorp process polls at the same time, rely on the existing ledger
  idempotency and surface only the same degraded/source-health states watch
  would surface;
- do not create a companion-specific ingestion path or cursor identity.

## Error Handling

If no pet exists, the launcher/native app should present a simple native
message directing the user to initialize Glorp from the CLI in V1. Native
onboarding/init is post-V1 unless separately approved. The empty state must not
panic or silently open an empty window.

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

Preview Lab round work is part of the first implementation slice. It is not a
post-AppKit cleanup task.

Useful fixtures:

- `round-normal`
- `round-active-pulse`
- `round-asleep-night`
- `round-helper-trouble`
- `round-flat-color`
- `round-glitch-dialect`
- `round-crystal-dialect`

The manifest should use a schema bump with first-class round metadata rather
than burying everything in ad hoc `inputs` keys:

- dimensions
- scenario kind, such as `round`
- target renderer, such as `preview-cells` or `native-reference`
- scene model inputs
- color capability
- privacy contract, especially which exact-count/text fields are excluded
- aperture/mask metadata, including safe radius and transparent/outside-mask
  regions
- review prompts

Cell artifacts alone are not enough for a round companion. Preview output must
include either an explicit aperture-mask artifact or mask metadata in the cell
artifact so tests can distinguish true outside-aperture transparency from blank
opaque cells.

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
The implementation plan should still include a local macOS smoke path covering:

- app bundle launch;
- CLI launcher exits after opening/focusing the app;
- no-env Dock/Finder launch can find helpers or reports a clear helper state;
- closing the companion window keeps the app running;
- Dock reopen restores/recreates the companion window;
- no-pet native empty state appears;
- companion can build a scene from real local state.

## Open Implementation Questions

These do not block the design direction, but they should be resolved during
implementation planning:

- Whether V1 needs any menu-bar affordance in addition to the required Dock
  lifecycle.
- Whether the release artifact is carried by the macOS npm platform package or
  a separate macOS artifact.
- Which helper-discovery option the Dock-launched app uses.
- Exact default/minimum window dimensions and window level.

## Recommendation

Spec and build V1 as native macOS companion first:

1. Define `RoundSceneModel`.
2. Define `RoundSceneLayout` and renderer-neutral scene moments.
3. Add Preview Lab round scenarios, schema/mask metadata, and acceptance checks.
4. Add a native macOS companion facade that launches like an app and uses the
   same state/polling model as watch.
5. Keep terminal round output out of V1 unless a hidden harness later proves
   necessary.

This keeps the product honest: the round feature is a polished companion
window, not a terminal trick. It also keeps future hardware plausible by
putting the reusable boundary at the semantic scene model instead of inside
AppKit.
