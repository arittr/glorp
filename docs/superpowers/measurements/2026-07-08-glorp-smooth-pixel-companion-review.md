# Smooth Pixel Companion Review

Date: 2026-07-08
Commit: 32c25bd
Reviewer: Drew Ritter
Machine: workerbee

## Preview Lab

Command:

```bash
cargo run --features dev-preview -- dev-preview --scenario pixel --out target/glorp-preview-pixel
```

Manifest schema: 8.

- pass: `pixel-fuzz-s3-content-idle` is present as a `pixel` scenario with `96 x 96` dimensions, `frames/pixel-fuzz-s3-content-idle.pixel.json`, schema `1`, `9216` RGBA pixels, species `fuzz`, stage `s3`, and mood `content`; browser capture showed the Fuzz S3 idle pixel pet rendered on the canvas.
- pass: `pixel-glitch-s4-feed-pulse` is present as a `pixel` scenario with `96 x 96` dimensions, `frames/pixel-glitch-s4-feed-pulse.pixel.json`, schema `1`, `9216` RGBA pixels, species `glitch`, stage `s4`, and mood `content`; browser capture showed the Glitch S4 feed-pulse fixture rendered on the canvas.
- pass: `pixel-idle`, `pixel-asleep-calm`, and `pixel-feed-pulse` are present as `pixel-animation` strips with `48` frames each, `34ms` frame duration, and target `companion.pixel.pet`; browser playback advanced visible frames to `pixel-idle-frame-047`, `pixel-asleep-calm-frame-036`, and `pixel-feed-pulse-frame-024`.
- pass: after final-review fixes, `pixel-feed-pulse` decays across its exported strip: Glitch accent-aura alpha total drops from `144827` at `frame-000` to `82577` at `frame-047`.
- pass: privacy scan over `frames/*.pixel.json` and `strips/*/*.pixel.json` found no matches for raw seed, source names, exact counts, file paths, project names, diagnostics, prompt text, response text, transcript text, user paths, `claude`, `codex`, or `agentsview`; pixel artifact keys were limited to `elapsed_ms`, `height`, `mood`, `pixels`, `schema_version`, `species`, `stage`, and `width`.

## Manual AppKit Review

- pass: Classic/default launch through `cargo xtask companion fresh` opened PID `71221` with CoreGraphics window id `4792` at `360x360`; window-only capture `target/glorp-classic-window.png` showed the classic companion animating inside the round HUD.
- pass: Pixel through `glorp companion --renderer pixel` opened PID `78752` with CoreGraphics window id `4806`; window-only capture `target/glorp-pixel-cli-window.png` showed the Pixel renderer in the app bundle path.
- pass: Pixel through `open -n target/macos/Glorp.app --args --renderer pixel` opened PID `76874` with CoreGraphics window id `4799`; window-only capture `target/glorp-pixel-window.png` showed the Pixel renderer in the direct app-bundle path.
- pass: default size review used the `360x360` Pixel window capture; the pixel pet was crisp at the default size and nearest-neighbor edges were visible rather than blurred.
- deferred: minimum size was not manually exercised; the window is configured with `MIN_WINDOW_SIZE = 260.0`, but AX/AppleScript window frame reads and writes failed for the borderless companion window, so no minimum-size capture was produced.
- deferred: resized window was not manually exercised; CoreGraphics could report the Pixel window bounds, but AX/AppleScript could not mutate the window frame, so no resized-window capture was produced.
- deferred: fullscreen was not manually exercised; the code configures `NSWindowCollectionBehavior::FullScreenPrimary`, but automation could not drive the hidden-titlebar window into fullscreen and no fullscreen capture was produced.
- pass: orientation is correct in the Pixel captures; the pet body, eyes, HUD text, and perimeter gauges render upright in both Pixel launch paths.
- pass: alpha/aperture review found no square Pixel frame corners in window-only captures; the pixel pet sits inside the circular companion aperture while the rounded outer window remains transparent outside its visible shell.
- pass: overlay/HUD preservation is visible in Pixel captures; perimeter gauges, the circular halo, HUD numbers, and text render above the Pixel frame instead of being hidden behind it.
- deferred: resize stale-frame behavior was not exercised because scripted resize was blocked by AX/AppleScript frame access failure; no stale-frame pass/fail visual capture exists.

## CPU

| Mode | PID | Command | Samples | Average CPU | Evidence |
| --- | ---: | --- | ---: | ---: | --- |
| Classic idle | 71221 | `top -pid "$classic_pid" -stats pid,command,cpu,time -l 12 -s 5` | 12 | 21.33% | `target/glorp-classic-idle-top.txt`; values `0.0,14.7,77.1,24.5,16.6,26.9,16.2,16.6,15.3,16.4,16.5,15.2` |
| Pixel idle | 78752 | `top -pid "$pixel_pid" -stats pid,command,cpu,time -l 12 -s 5` | 12 | 21.11% | `target/glorp-pixel-idle-top.txt`; values `0.0,16.8,18.3,18.4,18.2,17.1,18.4,19.0,63.4,18.3,27.4,18.0` |
| Classic active | blocked | same `top` command | blocked | blocked | No deterministic live usage pulse was available during this review session, so active review was not measured. |
| Pixel active | blocked | same `top` command | blocked | blocked | No deterministic live usage pulse was available during this review session, so active review was not measured. |

## Automated Gate

After final-review fixes, the local automated gate passed on macOS with:

```bash
cargo fmt --check
cargo test
cargo test --features dev-preview --test dev_preview
cargo test --features dev-preview dev_preview::scenarios
cargo test --features dev-preview dev_preview::export
cargo clippy --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features --all-targets
```

The final live-path regression also passed:

```bash
cargo test companion::app::tests::companion_pixel_tick_recomputes_pulse_age_between_polls
```

Linux portability still requires running the same command on Ubuntu before claiming Linux coverage.

## Accepted Opt-In Follow-Ups

- Pixel remains opt-in until minimum-size, resized-window, fullscreen, resize stale-frame, and active CPU behavior are manually exercised in an environment that can drive the borderless companion window.
- Active CPU review remains unmeasured because no deterministic live usage pulse was available in this session; idle CPU stayed within the Classic budget.

## Default Flip Decision

Pixel remains opt-in in this implementation.
