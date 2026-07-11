# Glorp Renderer Accessibility And Input Qualification

**Date:** 2026-07-10
**Status:** automated benchmark audit passes; **manual Accessibility Inspector/VoiceOver gate pending**
**Decision owner:** repository owner/user

## Scope

This report covers only the hidden synthetic `renderer-spike-wgpu` host. It does not claim that a production retained companion exists, and it does not replace the later real-companion accessibility audit.

Automated evidence is under:

`target/renderer-spikes/wgpu-qualified-accessibility/`

Accepted native runs:

- `static-360`
- `resize-360`
- `resize-720`
- `occlusion-360`

Each run used a corrected candidate-enabled arm64 binary, generated manifest/privacy/cleanup evidence, and passed `cargo xtask renderer-spike validate` at collection time. Because the startup and accessibility audit additions change the binary, these are qualification-development artifacts rather than the final Task 3 frozen-binary matrix; the new final binary must rerun the applicable tracks.

## Automated results

### Semantic tree and privacy

All accepted native runs report:

- exactly **1 group** (`Glorp habitat`);
- exactly **3 value nodes** (`Today`, `Pace`, `Daily comparison`);
- exactly **4 total semantic children**;
- no per-glyph accessibility children;
- sanitized synthetic names and values.

The semantic source remains `semantic_fixture`, which contains only deterministic synthetic values. The ordinary privacy scanner passes on every accepted run.

A separate seeded test writes `very-secret-seed` into an isolated accessibility artifact and proves that the renderer-spike privacy scanner rejects it:

```bash
cargo test --features renderer-spike \
  privacy_scanner_rejects_seeded_secret \
  --test renderer_spike -- --nocapture
```

Result: pass; the seeded private label is rejected.

### Bounds, resize, and backing scale

Initial native evidence exists at logical 360 and 720 with backing scale **2.0**.

The resize tracks record semantic bounds after native AppKit window changes. Observed logical sizes include 360, 480, and 720, all at backing scale 2.0:

- `resize-360`: 480 → 720 → 360 → 480
- `resize-720`: 360 → 480 → 720 → 360 → 480

The group bounds track the full logical square. The three HUD values retain deterministic scaled bounds from `semantic_fixture`.

### Actual NSEvent delivery

The benchmark now constructs native AppKit events with:

- `NSEvent::mouseEventWithType...` using `LeftMouseDown`;
- `NSEvent::keyEventWithType...` using `KeyDown`.

The events are sent through the Objective-C `mouseDown:` and `keyDown:` selectors on the real benchmark `NSView`. Accepted runs report both:

- `synthetic_mouse_event_delivered: true`
- `synthetic_key_event_delivered: true`

The mouse event projects the actual event location through `convertPoint:fromView:` before logical conversion. This is native delivery evidence, not merely a direct pure-function call.

### Stale generation/frame snapshots

The benchmark uses an explicit generation/frame freshness predicate. Deterministic unit evidence proves:

- current generation + current frame is accepted;
- stale generation is rejected;
- stale frame is rejected.

Every accepted native run records:

- `stale_snapshot_rejected: true`
- `current_snapshot_accepted: true`

This is a benchmark contract for future input snapshots; the synthetic click itself remains generation-neutral native delivery.

### Focus and cleanup

All accepted runs report that the benchmark view became first responder.

The occlusion run detaches accessibility children and clears their parents before `orderOut`, then restores parents and children after reveal:

- `hide_children_detached: true`
- `reveal_children_restored: true`

Every normal/fault-completing run clears view children and parent links during close:

- `close_children_detached: true`

The corrected qualification validator rejects wgpu runs that do not have the required semantic counts, sanitized values, native mouse/key delivery, stale/current snapshot results, first-responder status, or close cleanup. Occlusion validation additionally requires detach/restore.

## Automated limitations

The automated audit does **not** prove how Accessibility Inspector or VoiceOver presents the hierarchy, reading order, hit-testing, or announcements. It also does not establish full menu behavior from assistive technology.

The current hidden spike uses ordinary AppKit window chrome. Native keyboard-event acceptance and first-responder evidence pass, but product expectations for menu, quit shortcut, and fullscreen must be reviewed manually. These cannot be converted into an implementation-agent exception.

## Required manual checklist

A human reviewer must run the newly refrozen final qualification binary on the primary macOS machine and record:

- operator name;
- exact date;
- macOS version/build;
- Accessibility Inspector and/or VoiceOver version/tool;
- final binary path and SHA-256;
- logical size tested (360 and 720);
- screenshots where useful.

### Launch

Run the final binary through the qualification runner, using a duration long enough for review. Do not run from `target/release/glorp`.

### Accessibility hierarchy and reading

- [ ] Focus reaches one `Glorp habitat` group.
- [ ] The group exposes exactly `Today`, `Pace`, and `Daily comparison` values.
- [ ] No individual rendered glyph appears as a child.
- [ ] Names/values contain no source, project, file, prompt, response, diagnostic, or seeded private text.
- [ ] VoiceOver reads each value once and in a stable order.

### Bounds and hit testing

- [ ] At logical 360 / Retina 2×, focus bounds align with the window and three value regions.
- [ ] At logical 720 / Retina 2×, focus bounds align after resize.
- [ ] Pointer/hit testing selects the expected semantic region at the center and edges.
- [ ] Bounds remain correct after 360 → 480 → 720 → 360 resize cycling.

### Lifecycle

- [ ] Hide/occlude removes stale accessible children from traversal.
- [ ] Reveal restores exactly one group and three values, without duplicates.
- [ ] Close leaves no stale window/group/value in Accessibility Inspector.
- [ ] Injected callback/capture faults do not leave stale children or focus targets.

### Keyboard, menu, quit, fullscreen

- [ ] The window can become keyboard focus/first responder without trapping system navigation.
- [ ] Standard application menu behavior remains available.
- [ ] Quit shortcut behaves normally and cleanup completes.
- [ ] Fullscreen entry/exit does not duplicate or lose the semantic tree, if fullscreen is offered by the window.

### Record decision

The reviewer must record one unambiguous result:

- `PASS` with operator/date/tool/macOS/binary hash and completed checklist; or
- `FAIL` with the exact failed item and evidence.

Accessibility cannot use the plan's exception mechanism. If no human reviewer completes this checklist before the qualification timebox ceiling, the backend decision is **no selection**.

## Current gate status

- Automated semantic/bounds/native-input/staleness/cleanup/privacy evidence: **pass**.
- Human Accessibility Inspector/VoiceOver audit: **pending**.
- Overall mandatory accessibility/input gate: **pending; wgpu cannot yet be selected**.
