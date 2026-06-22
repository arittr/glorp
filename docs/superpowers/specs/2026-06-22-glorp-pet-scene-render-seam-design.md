# Glorp Pet Scene Render Seam — Design

- **Date:** 2026-06-22
- **Status:** Direction approved by Drew (Option C, two native surfaces, companion-first). Written for review before implementation planning.
- **Builds on / revises:**
  - `2026-06-15-glorp-presentation-architecture-design.md` — **completes** its intent. That spec designed `src/presentation/` as "a backend-neutral scene vocabulary layer" that all surfaces route through; the implementation stopped at a privacy/normalization filter. This spec realizes the original seam.
  - `2026-06-13-glorp-macos-round-companion-design.md` — **revises** it. That spec deliberately made the round companion a thin "porthole, not a shrunken watch screen." This spec upgrades it to a first-class surface that shares the full visual vocabulary, while **preserving** that spec's privacy boundary for informational content on external displays.

## Problem

Glorp renders the same pet to multiple surfaces — the `watch` TUI, the macOS round `companion`, the `menubar` popover, and the `dev-preview` golden harness — but the only thing those surfaces genuinely share is the raw pet body grid. Everything that makes a pet look *alive on screen* lives **above** the shared seam, hand-built imperatively inside the watch-only module tree `src/tui/panels/pet.rs` + `src/tui/panels/pet/` (~3,985 lines together, with `PetPanel::render` alone at ~1,697 lines).

Concretely, three structural defects:

1. **The shared contract ends one layer too early.** Every surface agrees on exactly one artifact: `vm.pet_art: Vec<String>` + `vm.pet_spans: Vec<StyledSegment>`, produced by `render_pet` and stored by `rerender_pet_for_view_model`. That is the body grid (silhouette, mood expression, blink, glitch corruption, particles, S6 sparkle). Grounding, contact shadow, ambient/motes, props and reactions, speech, cursor-eyes, twinkle/shimmer/token-pop, phase/activity tint, and the halo all live above it, callable only by watch. The companion **physically cannot inherit them** — it re-implements a thinner parallel stack (`src/round/{model,layout,draw}.rs`, ~900 lines of render path) that drops every effect that was not already a field on the view model.

2. **Role → color is resolved three independent times.** `src/tui/panels/pet/colors.rs::pet_role_style` (watch), `src/companion/app.rs::pet_role_color` (companion), and `src/menubar/render.rs::role_color_for_profile` (menubar) each wrap `src/pet/palette.rs::role_color` and add their own tint/bold/dim. The `Corruption` role is already special-cased watch-only (remapped to `pet_accent`), proving the paths drift on every new role.

3. **Per-frame effects are computed inline and are invisible to other surfaces.** `wander`, `facing`, `twinkle`, `shimmer`, and `token_pop` are computed inside `PetPanel::render` (`pet.rs:417-424`) and either used transiently or written to a *cloned* view model — never stored where another surface can read them.

The cost shows up every time we add a feature. The recent grounding/contact-shadow series touched 8+ watch files (including positional assertion tests in `pet_scene.rs`/`watch_screen.rs` and the `habitat.rs` game model) and **still** left the companion untouched. That is the "we shouldn't have to work this hard" tax, paid in full.

The forcing function: the companion is about to become a real display surface, dragged onto a small external monitor (Napster View 2.1″; generic round HDMI panels such as the Elecrow 2.8″/480×480 — all plug-and-play monitors macOS sees as ordinary external displays). It must be **as good or better than watch**, not a thin dashboard.

## Goals

1. **One render seam every surface shares.** A pet looks identical across surfaces except where a surface *declares* a difference. Adding the next grounding-style feature happens **once** and every surface inherits it.
2. **Kill the triple color path** — a single role→color resolver; per-surface differences become explicit, typed policy rather than scattered conditionals.
3. **Make per-frame effects data, not inline side effects** — so any surface can read them, and so the renderer stays content-agnostic.
4. **Upgrade the companion to first-class** visual richness (grounding, ambient, props, speech, effects, halo, mood color) while **preserving the external-display privacy boundary** for informational content.
5. **Backend-agnostic, serializable draw output** so today's two AppKit surfaces (round + rectangular), the terminal, the menubar, and a possible future image/web target are all dumb blitters over the same artifact.
6. **No regression to the locked invariants** — `art.rs` 11×8 templates untouched, `dev-preview` goldens preserved as the regression net (re-baked only by reviewed, intentional change), only-real-data and tamagotchi-spirit intact.

## Non-goals

- **Not** touching `src/pet/render.rs::render_pet` or the `art.rs` 11×8 templates. The seam lives strictly *above* `render_pet`; the body grid stays exactly as-is.
- **Not** moving layout/area/cursor concerns *into* `render_pet`. Wander depends on viewport width and the cursor depends on the mouse; pushing them down would violate the "renderer is content-agnostic" rule. They resolve *above* `render_pet`, in the seam.
- **Not** adding new animation features, and **not reviving** the moment system — the dead `RoundSceneMoment` and `RoomLifeProfile.scene_moments` scaffolding is deleted in Track 7, not preserved as a future hook. Wiring real "moment" animations is a separate future spec.
- **Not** building a web/canvas adapter or an SPI/framebuffer adapter now. The draw list stays serializable so either is later a dumb blitter; we build only the native surfaces in scope.
- **Not** weakening the round/external-display privacy redaction. Visual richness increases; informational disclosure does not.

## Direction / Product Model

There is one semantic scene, one policy object per surface, and one resolved draw list. Surfaces are dumb blitters.

```
PetState
  └─ build_watch_view_model(state, db)            [unchanged data layer: vitals, day_context,
       │                                            life_profile, render_pet → pet_art/spans/palette]
       └─ PetScene::build(vm, now)                [compose the scene ONCE — absorbs grounding.rs,
            │                                       ambient.rs, props, performance, speech, halo,
            │                                       AND the inline compute_* effects from PetPanel]
            │
            ├─ scene.render(WATCH_STYLE,  term_viewport)  → SceneDrawList → ratatui Buffer blit
            ├─ scene.render(ROUND_STYLE,  round_viewport) → SceneDrawList → AppKit + circular clip
            ├─ scene.render(SCREEN_STYLE, win_viewport)   → SceneDrawList → AppKit window (new)
            └─ scene.render(MENU_STYLE,   menu_viewport)  → SceneDrawList → NSAttributedString
```

This is the data flow the 06-15 spec already prescribed (`WatchViewModel + now → PresentationScene → SurfaceSpec + capabilities → draw commands → adapters`). `PetScene` realizes its `PresentationScene`; `SurfaceStyle` realizes its `SurfaceSpec + capabilities`; `SceneDrawList` realizes its "draw commands." We are finishing that design, not replacing it.

## Architecture

Three types, one home: `src/presentation/`.

### 1. `PetScene` — semantic, surface-independent

Built **once** per frame from the view model. Knows nothing about ratatui, AppKit, colors-as-RGB, or viewport size. Contains *placements and intent*, not pixels.

```rust
// src/presentation/scene.rs  (evolved from today's PresentationScene)
struct PetScene {
    pet: Vec<PetCell>,          // { row, col, glyph, role: PaletteRoleName }  — from render_pet
    grounding: Grounding,       // feet anchor row + contact-shadow footprint  — from grounding.rs
    ambient: Vec<AmbientGlyph>, // sky / motes / activity glyphs               — from ambient.rs
    props: Vec<PropPlacement>,  // earned-prop layers + reaction intent        — from props.rs / life.rs
    overlays: Vec<Overlay>,     // enum { Speech{..}, Halo{..}, Performance{..} } — halo now shared
    effects: EffectState,       // wander PHASE (never a resolved x), facing intent, twinkle, shimmer, token_pop
    palette: ResolvedPalette,   // existing src/pet/palette.rs type: per-role resolved Rgb
                                //   (body/eye/mouth/accent/pattern/particle/corruption); mood-eye already baked
    room: RoomContext,          // biome, dialect, day_phase, work_weather
    privacy: PrivacyFacts,      // raw facts (source names, exact counts) that SurfaceStyle may redact
}
```

`EffectState` holds the **semantic** effect, never a resolved pixel. Wander is a phase/instant plus idle minutes, not an `x`. This is what keeps `PetScene` surface-independent and keeps viewport-dependent math out of `render_pet`.

**Neutral-id constraint (from 06-15, retained):** `PetScene` and `SceneDrawList` must not depend on `tui::component::TargetPath` or any watch-specific id. Target/overlay ids are neutral (`SurfaceTargetId`). Watch adapters may map neutral ids to `TargetPath`; the seam must not.

### 2. `SurfaceStyle` — the only place surfaces differ

Per-surface policy, applied at one boundary. Replaces the scattered conditionals and the three color wrappers. Includes the **privacy projection** (subsuming today's `PrivacyProjection`), so a surface's informational disclosure is part of its declared policy.

```rust
// src/presentation/surface.rs
struct SurfaceStyle {
    detail: Detail,             // Full | Compact | Minimal  (was RoundDetailLevel)
    clip: Clip,                 // None | Circle
    // color policy (the three wrappers, factored into explicit knobs)
    phase_tint: bool,           // dusk warm / night cool+dim  (tint_pet_styles_for_phase)
    energy_droop: bool,         // unify watch's energy×perf with menubar's flat sleep dim
    activity_lift: bool,        // needs live-activity context
    shimmer: bool,              // motion feedback: shimmer / token-pop brighten
    prop_reaction: bool,        // prop reaction lift
    eye_emphasis: EyeEmphasis,  // None | TerminalBold | Brightness  (capability-aware)
    source_accent: bool,        // menubar's work-identity override on Accent/Particle
    // informational privacy (subsumes presentation::PrivacyProjection)
    privacy: PrivacyPolicy,     // source_names_visible, exact_counts_visible, project_context_visible, …
}

// The four concrete policies live in src/presentation/surface.rs. Example:
const WATCH_STYLE: SurfaceStyle = SurfaceStyle {
    detail: Detail::Full, clip: Clip::None,
    phase_tint: true, energy_droop: true, activity_lift: true,
    shimmer: true, prop_reaction: true, eye_emphasis: EyeEmphasis::TerminalBold,
    source_accent: false, privacy: PrivacyPolicy::FULL,
};
// ROUND_STYLE / SCREEN_STYLE: detail Compact|Full, clip Circle|None, eye_emphasis Brightness,
//   privacy: PrivacyPolicy::EXTERNAL (source names + exact counts redacted).
// MENU_STYLE: phase_tint/activity_lift/shimmer false, source_accent true, privacy FULL.
```

The `Corruption → pet_accent` remap is **not** a `SurfaceStyle` knob — it is a watch-only detail folded into the resolver's shimmer-brighten path (the only place it fires today); other surfaces resolve `Corruption` to its own palette color.

`eye_emphasis` is **capability-aware**, not a boolean: a terminal applies `Modifier::BOLD`; AppKit (no terminal modifiers) applies a brightness/weight bump. This is how the companion and screen surfaces finally emphasize eyes — today they get nothing.

### 3. `SceneDrawList` — resolved, backend-agnostic, serializable

The single artifact every adapter consumes and the single thing `dev-preview` serializes. Generalizes today's `RoundDrawCommand` + `RoundDrawKind` and subsumes `dev_preview::export::PreviewCell` (which already carries resolved `fg/bg` hex, `modifiers`, and an `outside_aperture` flag — the circular-clip concept already exists at the cell level).

```rust
// src/presentation/draw_list.rs
impl PetScene {
    fn render(&self, style: &SurfaceStyle, viewport: Viewport) -> SceneDrawList;
}

struct SceneDrawList {
    cells: Vec<DrawCell>,       // generalizes RoundDrawCommand + PreviewCell
    overlays: Vec<DrawOverlay>, // resolved speech box / halo ring / performance pip
}

struct DrawCell {
    row: u16, col: u16,         // grid space; adapters map to their output coordinate space
    glyph: char,
    rgb: Rgb,
    layer: Layer,               // Background | Room | Pet | Prop | Overlay
    modifiers: Modifiers,       // bold/italic/… ; AppKit ignores what it cannot express
    outside_aperture: bool,     // set when viewport.aperture culls this cell (circular surfaces)
}

struct Viewport {
    width_px: u32, height_px: u32,   // pixel dims for AppKit / image targets
    cell_size: CellSize,             // grid→pixel mapping
    aperture: Option<Aperture>,      // inscribed circle for round surfaces
}
enum CellSize { Terminal, AppKit(f32) }                        // f32 = pixels per glyph box
struct Aperture { center_x: f32, center_y: f32, radius: f32 }
```

`render` takes **two** inputs deliberately. `viewport` is per-frame (resolves position-dependent things — notably wander against `width`, and the circular cull) and lets the round adapter be **resolution-adaptive**: it reads the connected monitor's reported size rather than hardcoding, so any round panel "just works." `style` is mostly static policy. The terminal adapter works in integer cell space; AppKit/round/image adapters work in `f32` pixel space (exactly as `round/draw.rs` does today with `RoundAperture { width, height, center_x, center_y, radius }`).

### Adapters (dumb blitters)

| Surface | Adapter (becomes) | Notes |
|---|---|---|
| Watch TUI | `src/tui/panels/pet.rs` | `SceneDrawList` → ratatui `Buffer`. `PetPanel::render` shrinks from ~1,697 lines to a blitter. |
| Round companion | `src/companion/app.rs` | `SceneDrawList` → AppKit, `style.clip = Circle`, `viewport.aperture` culls. Replaces `derive_round_scene_model` → `build_draw_commands`. Resolution-adaptive. |
| Screen window (new) | `src/companion/` (shared blitter) | Full-size AppKit window, no clip. "As good or better than watch, on a monitor." |
| Menubar | `src/menubar/render.rs` | Pet-layer cells → `NSAttributedString` with `source_accent` knob. |
| dev-preview | `src/dev_preview/` | Serializes `SceneDrawList`; unifies `cells.json` + `round-commands.json`. |

## Surfaces and their `SurfaceStyle`

The intentional vs. accidental color differences (from the current code audit) become explicit policy. Intentional restraint is preserved; accidental drift is eliminated.

| Knob | Watch | Round companion | Screen window | Menubar | Rationale |
|---|---|---|---|---|---|
| `detail` | Full | Compact (by panel size) | Full | Minimal | round panels are tiny |
| `clip` | None | Circle | None | None | physical round panel |
| `phase_tint` | on | on | on | **off** | menubar is static UI (intentional) |
| `energy_droop` | on | on | on | on (unified math) | reconciles watch energy×perf with menubar flat dim |
| `activity_lift` | on | on | on | off | menubar has no live-activity context |
| `shimmer` | on | on | on | off | menubar is a snapshot |
| `prop_reaction` | on | on | on | off | prop reaction glows; menubar is a snapshot |
| `eye_emphasis` | TerminalBold | Brightness | Brightness | None | capability-aware; companion gains emphasis |
| `source_accent` | off | off | off | **on** | menubar surfaces work identity (intentional) |
| `privacy.source_names_visible` | true | **false** | **false** | true | external displays must not read as surveillance |
| `privacy.exact_counts_visible` | true | **false** | **false** | true | per 06-13 external-display boundary |

**Privacy revision (conscious, flagged for confirmation).** The round companion and the screen window are external-display surfaces (the screen window is a full-size rectangular AppKit window intended for an external monitor, treated identically to the round companion for redaction; internal-display use would revisit this — see Open Questions). They gain the full *visual* vocabulary (the 06-13 "porthole" visual restraint is lifted — Drew wants "as good or better"), but they **retain** the 06-13/06-15 *informational* redaction: no exact source display names, no exact counts, no project/file context. Speech bubbles remain pet flavor text driven by real signals (consistent with only-real-data), never transcript content. This is the one place "share all the same stuff" is intentionally bounded; see Open Questions.

## Effects as data

The inline `compute_*` calls move out of `PetPanel::render` into `PetScene::build` (semantic) and `render` (viewport resolution):

| Effect | Today | After |
|---|---|---|
| wander | `compute_wander_position_x(area.width, …)` inline, written to cloned vm | `EffectState.wander` (phase) in `build`; resolved to `x` in `render(viewport)` |
| facing | `compute_facing(area.width, …)` inline | `EffectState.facing` intent in `build`; mirror resolved in `render` |
| twinkle | `compute_twinkle(…)` transient | `EffectState.twinkle` spec in `build` |
| shimmer | `compute_shimmer_role(…)` transient | `EffectState.shimmer` role in `build`, applied per `style.shimmer` |
| token_pop | `profile_token_pop(…)` transient | `EffectState.token_pop` in `build`, applied per `style.shimmer` |

Net effect: the companion (and every surface) reads the same effect data; nothing is recomputed per surface; `render_pet` never learns about `area` or the cursor.

## dev-preview / golden contract

Today `dev-preview` exercises the **real** paths and serializes two resolved formats:
- Watch: `build_watch_view_model_at` → `render_watch_frame_with_layout` → `cells.json` (`PreviewCell { x, y, symbol, display_width, continuation, fg, bg, modifiers, outside_aperture }`), plus `layout.json`, `scene.json`, `room.txt`.
- Round: `derive_round_scene_model` → `layout_round_scene` → `build_draw_commands` → `round-commands.json` (`PreviewRoundCommandArtifact { kind, x, y, radius, label, text_len, span_count, color_rgba }`), plus `round-layout.json`, `scene.json`.

After unification both surfaces produce a `SceneDrawList`, so the two formats **collapse into one** serialized artifact (working name `*.scene-draw-list.json`); `cells.json` may be retained as a terminal-projection view if convenient. Schema versions bump where the artifact changes (`export.rs` `SCHEMA_VERSION` 4→5; `contract.rs` `CONTRACT_SCHEMA_VERSION` if contract artifacts change; `round-commands` retired). All regeneration happens **behind the existing ownership guard** (`.glorp-preview` marker + `PRODUCER = "glorp-dev-preview"`), so a re-bake can only overwrite glorp-owned preview output. Re-bakes are staged per surface and reviewed as their own commits.

## Architecture options considered

- **Option A — fully-resolved build per surface.** `PetScene::build(vm, surface, now)` does everything and emits a resolved draw list; adapters are dumb. Rejected: surface conditionals creep back *into* composition (`if surface == menubar …`), re-growing the god-function we are removing.
- **Option B — role-tagged list, adapter-side color.** Scene carries roles + effect state (no RGB); each adapter resolves color via one shared `resolve(role, palette, style)`. Rejected as the destination (kept as the on-ramp): drift is prevented only by convention (three call sites), and the scene is a half-value (not serializable end-to-end).
- **Option C — two layers: semantic `PetScene` → `SceneDrawList`, dumb adapters. ✅ Decision.** Composition is surface-blind; resolution happens once at the `render` boundary; surface variance is one typed `SurfaceStyle`; adapters cannot drift; the resolved draw list is the single serializable golden artifact. Costs one extra type and a little ceremony, justified by ≥2 live surfaces (plus a possible third) and golden-stability. Phase 1 of the rollout *is* Option B, so the on-ramp is not wasted.

## Implementation plan (tracks)

Strangler-fig, companion-first. Each track is independently shippable; behavior-preserving except where a golden re-bake is explicitly reviewed. `render_pet` and `art.rs` are **forbidden changes** in every track.

### Track 0 — Safety net
- **Purpose:** make the rest safe.
- **Scope:** pin current `cells.json` + `round-commands.json` as the regression baseline. Add a cross-surface **characterization test** capturing today's role→RGB for each surface × mood × role.
- **Verification:** `cargo test`, `cargo test --features dev-preview --test dev_preview`.
- **Stop condition:** baseline reproduces deterministically; characterization test green.

### Track 1 — One resolver + `SurfaceStyle` (the Option-B on-ramp)
- **Purpose:** kill the triple color path.
- **Scope:** factor the single role→RGB resolver out of `colors.rs`; express watch/companion/menubar as `SurfaceStyle` values routed through it. Knobs set to **reproduce today exactly**. Reconcile the sleep-dim divergence into the `energy_droop` knob; fold the watch-only `Corruption → pet_accent` shimmer quirk into the resolver (not a separate knob).
- **Forbidden:** any visible color change (goldens must stay byte-stable).
- **Verification:** goldens byte-stable; characterization test green; `cargo clippy --all-targets --all-features -- -D warnings`.
- **Stop condition:** all three surfaces resolve through one function; no golden diff.

### Track 2 — Lift effects to data
- **Purpose:** make per-frame effects readable by any surface; keep `render_pet` content-agnostic.
- **Scope:** move inline `compute_*` (wander/facing/twinkle/shimmer/token_pop) and grounding/ambient/props/performance placement out of `PetPanel::render` into `PetScene::build`; wander resolved against viewport in a `render` step. Watch reads from the scene.
- **Forbidden:** behavior change.
- **Verification:** watch goldens stable; visual diff via Preview Lab unchanged.
- **Stop condition:** `PetPanel::render` no longer computes effects inline.

### Track 3 — `SceneDrawList` + migrate companion ← **the visible win**
- **Purpose:** companion becomes first-class.
- **Scope:** introduce `SceneDrawList` and `render(style, viewport)`; point the companion AppKit blitter at `scene.render(ROUND_STYLE, viewport)`, retiring `derive_round_scene_model` → `build_draw_commands`. Companion inherits grounding/ambient/props/speech/effects/halo + mood color, under a round `SurfaceStyle` (Compact detail, Circle clip, external-display privacy).
- **Expected change:** companion gets richer (intended). `round-commands.json` → unified artifact, re-baked and reviewed.
- **Verification:** `dev-preview` round previews reviewed; companion runs on a 480×480 viewport.
- **Stop condition:** companion renders the full scene; old round draw path deleted.

### Track 4 — Migrate watch
- **Scope:** gut `PetPanel::render` into a `SceneDrawList` → `Buffer` blitter.
- **Expected change:** watch `cells.json` re-bakes only where unification intentionally shifts color; reviewed per surface.
- **Stop condition:** `src/tui/panels/pet/` is adapter-thin; watch visual review passes.

### Track 5 — Migrate menubar
- **Scope:** `append_pet` reads pet-layer cells with `MENU_STYLE` (source-accent on, phase off).
- **Stop condition:** menubar resolves through the seam; no intended visual change.

### Track 6 — Screen-window adapter (new surface)
- **Scope:** full-size AppKit window, no clip, reusing the companion blitter.
- **Stop condition:** a rectangular window renders the full scene at arbitrary size.

### Track 7 — Unify dev-preview + cleanup
- **Scope:** serialize `SceneDrawList` as the unified artifact; bump schema versions behind the ownership guard; delete dead scaffolding — `RoundSceneMoment`, `RoomLifeProfile.scene_moments`, `src/presentation/props.rs`, dead `PresentationSurface` variants, `derive_round_scene_model` field-copy, `round/draw.rs::build_draw_commands`.
- **Stop condition:** one golden format; no dead scaffolding; `cargo clippy … -D warnings` clean.

## Testing and review strategy

- **`art.rs` 11×8 invariant stays green throughout** — `render_pet` is never modified; the invariant tests (`art.rs` band/width/line tests) are untouched.
- **Golden net is the spine** — before each track current goldens reproduce; after, any diff is an intentional, reviewed re-bake, staged per surface so watch's color shift is never tangled with companion's.
- **Anti-drift parity test (the thing we never had)** — assert `PetScene::render` shares an identical *base* resolution across surfaces and differs **only** by declared `SurfaceStyle` knobs. This is what structurally prevents the next "companion forgot feature X."
- **Privacy test** — assert external-display `SurfaceStyle`s never emit source names / exact counts / project context into the draw list or overlays.
- **TDD per track** — each extraction is red-green-refactor against the characterization tests; visual review via `dev-preview`.

## Error handling

The seam is **pure and infallible**. `PetScene::build` and `render` are total functions over owned data — no I/O, no `Result`. All fallibility stays where it already lives (`build_watch_view_model` reading SQLite returns `Result`). Adapters paint cells infallibly. The re-arch therefore *shrinks* the error surface. The one new concern — degenerate viewports (window smaller than the pet; aperture tighter than the silhouette) — is handled by graceful clamp/cull (drop to `Detail::Minimal`, cull cells outside the aperture), matching today's `RoundDetailLevel::Minimal`, never an error.

## Migration rules

- `src/pet/render.rs` and `src/pet/art.rs` are **frozen** for this work. If a feature seems to need a `render_pet` change, stop and reconsider — it belongs in the seam.
- `src/presentation/` must not depend on `tui::component::TargetPath` or other watch-specific ids (06-15 constraint).
- Golden re-bakes only via the `dev-preview` command, only into `.glorp-preview`-owned output, one surface per reviewed commit.
- No new flavor content and no countdowns/ETAs introduced by this plumbing; the halo stays bucketed (Low/Med/High). Only-real-data and tamagotchi-spirit hold.
- Commit per track; keep `cargo fmt --check` and `cargo clippy --all-targets --all-features -- -D warnings` clean at each commit.

## Success criteria

1. A new pet visual feature (the next "grounding") is added in one place (`PetScene`/`render`) and appears on watch, companion, screen, and menubar without per-surface work.
2. Exactly one role→color resolver exists; the parity test enforces only-declared-knobs-differ.
3. The companion renders grounding, ambient, props, speech, effects, halo, and mood color on a 480×480 round panel, with external-display privacy intact.
4. A rectangular screen window renders the full scene at arbitrary size.
5. `PetPanel::render` and `src/round/` are adapter-thin; `derive_round_scene_model` and `build_draw_commands` are gone.
6. `dev-preview` emits one unified draw-list golden format; `art.rs` invariant tests never changed.
7. A new visual feature added to `PetScene` (e.g. a test-only `Overlay` variant) renders on all four surfaces with **no** change to any surface adapter, proven by one feature test exercising all surfaces.

## Open implementation questions

These do not block the direction; flag before/within planning.

1. **Privacy revision confirmation.** Confirm the bounded reading of "share all the same stuff": full *visual* richness on external surfaces, but informational redaction (no source names / exact counts / project context) retained per 06-13/06-15. Is that the intent, or do you want external surfaces fully unredacted because the display is personal? (And if the screen window ever lives on your *internal* display, should it relax to watch-level disclosure?)
2. **Moments deletion.** Confirm deleting both dead moment systems (`RoundSceneMoment`, `RoomLifeProfile.scene_moments`) rather than keeping them as a future hook.
3. **Golden re-bake aggressiveness.** Track 4 may shift some watch `cells.json` RGB even when visually identical (rounding/order). Preference: one reviewed re-bake commit per surface, or hold watch byte-stable by pinning the exact legacy resolution math?
4. **Unified artifact shape.** Keep `cells.json` as a terminal-projection alongside `*.scene-draw-list.json`, or fully replace it (larger manifest change, simpler contract)?
5. **Screen-window scope in this pass.** Build the rectangular window adapter now (Track 6) or defer it until the companion lands and the round surface is proven?

## Recommendation

Proceed with Option C via the companion-first strangler-fig. Tracks 0–2 are behavior-preserving and de-risk the rest; Track 3 delivers the visible companion win early; Tracks 4–7 generalize and clean up. The design completes the 06-15 presentation seam and consciously upgrades the 06-13 round companion to first-class while keeping its privacy boundary — resolving both of Drew's complaints ("they look totally different" = color/effect drift; "companion missing core features" = the watch-only render tree) at the structural root.
