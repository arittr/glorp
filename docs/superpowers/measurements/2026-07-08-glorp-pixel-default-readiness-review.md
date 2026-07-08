# Pixel Default-Readiness Review

Date: 2026-07-08
Branch: `codex/assess-smooth-3d-pet-motion-p2`
Reviewed diff base: `5c8b2b2`
Head before final-review fixes: `054ff10`
Final-review fixes: included in this branch after whole-branch review
Reviewer: Drew Ritter
Machine: workerbee

## Preview Lab

Command:

```bash
cargo run --features dev-preview -- dev-preview --scenario pixel --out target/glorp-preview-pixel-readiness
```

Manifest path: `target/glorp-preview-pixel-readiness/manifest.json`

- Hero side-by-side review surface: `target/glorp-preview-pixel-readiness/index.html`
- Hero review summary: `target/glorp-preview-pixel-readiness/review.md`
- Hero artifacts:
  - `target/glorp-preview-pixel-readiness/frames/pixel-fuzz-s3-content-idle.txt`
  - `target/glorp-preview-pixel-readiness/frames/pixel-fuzz-s3-content-idle.pixel.json`
  - `target/glorp-preview-pixel-readiness/frames/pixel-glitch-s4-feed-pulse.txt`
  - `target/glorp-preview-pixel-readiness/frames/pixel-glitch-s4-feed-pulse.pixel.json`
- Sidecar artifacts:
  - `target/glorp-preview-pixel-readiness/frames/pixel-fuzz-s3-content-idle.pixel-art.json`
  - `target/glorp-preview-pixel-readiness/frames/pixel-fuzz-s3-content-idle.pixel-fit.json`
  - `target/glorp-preview-pixel-readiness/frames/pixel-glitch-s4-feed-pulse.pixel-art.json`
  - `target/glorp-preview-pixel-readiness/frames/pixel-glitch-s4-feed-pulse.pixel-fit.json`
  - `target/glorp-preview-pixel-readiness/frames/pixel-species-matrix.pixel-art.json`
  - `target/glorp-preview-pixel-readiness/frames/pixel-species-matrix.pixel-fit.json`
- Fullscreen-equivalent fit evidence:
  - `target/glorp-preview-pixel-readiness/frames/pixel-fuzz-s3-content-idle.txt`
  - `target/glorp-preview-pixel-readiness/frames/pixel-fuzz-s3-content-idle.pixel-fit.json`
  - `target/glorp-preview-pixel-readiness/frames/pixel-glitch-s4-feed-pulse.txt`
  - `target/glorp-preview-pixel-readiness/frames/pixel-glitch-s4-feed-pulse.pixel-fit.json`
- Result: pass. The bundle wrote schema version `8`, listed Pixel frame, art, and fit artifacts, recorded `round::pixel_fit::pixel_companion_fit` in Pixel fit metadata, and the Pixel fit sidecars record schema version `2` with `geometry_evidence` rows for min/default/large/fullscreen review sizes. This is bundle-contract evidence, not a recorded human art signoff.

## AppKit Review

| Surface | Launch command | PID | Window id | Evidence path | Result | Notes |
| --- | --- | ---: | ---: | --- | --- | --- |
| Classic default | `cargo xtask companion fresh` | 63277 | 5421 | `target/task6-artifacts/classic-default-window.png`; `target/task6-artifacts/classic-default-geometry.txt` | pass | Window capture is nonblank and shows the Classic renderer inside the round HUD. |
| Pixel 360x360 | `open -n target/macos/Glorp.app --args --renderer pixel --review-size 360x360` | 65265 | 5467 | `target/task6-artifacts/pixel-360-window.png`; `target/task6-artifacts/pixel-360-windows.txt` | pass | CoreGraphics bounds recorded `360x360`; runtime screenshot and geometry are native visual evidence. Preview fit sidecars provide the portable fit contract for the same review size. |
| Pixel 260x260 | `open -n target/macos/Glorp.app --args --renderer pixel --review-size 260x260` | 65514 | 5481 | `target/task6-artifacts/pixel-260-window.png`; `target/task6-artifacts/pixel-260-windows.txt` | pass | CoreGraphics bounds recorded `260x260`; runtime screenshot and geometry are native visual evidence. Preview fit sidecars provide the portable fit contract for the same review size. |
| Pixel 480x480 | `open -n target/macos/Glorp.app --args --renderer pixel --review-size 480x480` | 65586 | 5494 | `target/task6-artifacts/pixel-480-window.png`; `target/task6-artifacts/pixel-480-windows.txt` | pass | CoreGraphics bounds recorded `480x480`; runtime screenshot and geometry are native visual evidence. Preview fit sidecars provide the portable fit contract for the same review size. |
| Pixel active pulse | `open -n target/macos/Glorp.app --args --renderer pixel --review-size 360x360 --review-active-pulse` | 65716 | 5507 | `target/task6-artifacts/pixel-active-window.png`; `target/task6-artifacts/pixel-active-windows.txt` | pass | Launch succeeds on the hidden active-pulse path; runtime screenshot and geometry are native visual evidence. Preview fit sidecars provide the portable fit contract for the same review size. |

## CPU

Protocol:

```bash
top -pid "$classic_pid" -stats pid,command,cpu,time -l 12 -s 5
top -pid "$pixel_pid" -stats pid,command,cpu,time -l 12 -s 5
top -pid "$classic_active_pid" -stats pid,command,cpu,time -l 12 -s 5
top -pid "$pixel_active_pid" -stats pid,command,cpu,time -l 12 -s 5
sample "$pixel_pid" 10 -file target/glorp-pixel-idle-sample.txt
sample "$pixel_active_pid" 10 -file target/glorp-pixel-active-sample.txt
```

Average and p95 use kept samples only, excluding the first `top` sample from each run.

| Mode | PID | Raw top artifact | Sample artifact | Kept sample count | Average CPU | p95 CPU | Budget | Result |
| --- | ---: | --- | --- | ---: | ---: | ---: | --- | --- |
| Classic idle | 69710 | `target/glorp-classic-idle-top.txt` | none | 11 | 28.02 | 60.9 | baseline | pass |
| Pixel idle | 69808 | `target/glorp-pixel-idle-top.txt` | `target/glorp-pixel-idle-sample.txt` | 11 | 24.10 | 43.9 | avg delta must be no more than `+5.0`; p95 delta must be no more than `+10.0` vs Classic idle | pass |
| Classic active | 66078 | `target/glorp-classic-active-top.txt` | none | 11 | 22.65 | 36.0 | baseline | pass |
| Pixel active | 65716 | `target/glorp-pixel-active-top.txt` | `target/glorp-pixel-active-sample.txt` | 11 | 23.32 | 37.4 | avg delta must be no more than `+5.0`; p95 delta must be no more than `+10.0` vs Classic active | pass |

CPU notes:

- Pixel idle delta vs Classic idle: average `-3.92`, p95 `-17.0`
- Pixel active delta vs Classic active: average `+0.67`, p95 `+1.4`
- Pixel idle sample path: `target/glorp-pixel-idle-sample.txt`
- Pixel active sample path: `target/glorp-pixel-active-sample.txt`

## Default-Readiness Gates

| Gate | Artifact path | Result | Notes |
| --- | --- | --- | --- |
| Portable fit contract | `target/glorp-preview-pixel-readiness/frames/pixel-fuzz-s3-content-idle.txt`; `target/glorp-preview-pixel-readiness/frames/pixel-fuzz-s3-content-idle.pixel-fit.json`; `target/glorp-preview-pixel-readiness/frames/pixel-glitch-s4-feed-pulse.txt`; `target/glorp-preview-pixel-readiness/frames/pixel-glitch-s4-feed-pulse.pixel-fit.json` | pass | The preview text fixtures contain `fit fullscreen ready`, and schema `2` sidecars record machine-readable `geometry_evidence` for min/default/large/fullscreen with `producer = round::pixel_fit::pixel_companion_fit`. This is the portable fit contract, not proof that AppKit's `NSAttributedString` measurement is byte-identical. |
| HUD body overlap | `target/glorp-preview-pixel-readiness/frames/pixel-fuzz-s3-content-idle.pixel-fit.json`; `target/glorp-preview-pixel-readiness/frames/pixel-glitch-s4-feed-pulse.pixel-fit.json`; `target/task6-artifacts/pixel-260-window.png`; `target/task6-artifacts/pixel-480-window.png` | pass | Preview fit sidecars report `body_eye_mouth_pixels = 0` and `translucent_effect_pixels = 0` for every review geometry; captured windows keep the body above the native HUD block. The native text path still needs visual review because AppKit measures and lays out text at runtime. |
| Cast identity | `target/glorp-preview-pixel-readiness/frames/pixel-fuzz-s3-content-idle.pixel-art.json`; `target/glorp-preview-pixel-readiness/frames/pixel-glitch-s4-feed-pulse.pixel-art.json`; `target/glorp-preview-pixel-readiness/frames/pixel-species-matrix.txt`; `target/task6-artifacts/pixel-260-window.png`; `target/task6-artifacts/pixel-360-window.png` | blocked | Structural artifacts show identity cues exist in the Pixel reference data. The species matrix is a representative roster/coverage fixture, not side-by-side visual proof for every species/stage, and this review did not record a reviewer-approved judgment that cues stay legible enough in Preview or AppKit. |
| All species/stages smoke | `target/task6-logs/cargo-test.log`; `target/task6-logs/dev-preview-test.log`; `target/glorp-preview-pixel-readiness/frames/pixel-species-matrix.pixel.json` | pass | `cargo test` passed `all_species_all_stages_render_reference_driven_frames`; dev-preview passed Pixel bundle contract coverage; species matrix artifact was generated. This is renderability coverage, not visual identity approval. |
| Active pulse path | `target/task6-artifacts/pixel-active-window.png`; `target/task6-artifacts/pixel-active-windows.txt`; `target/task6-logs/cargo-test.log` | pass | Hidden `--review-active-pulse` launch path produced a live Pixel capture, and the suite includes `review_burst_signal_uses_live_burst_path`. |
| CPU budget | `target/glorp-classic-idle-top.txt`; `target/glorp-pixel-idle-top.txt`; `target/glorp-classic-active-top.txt`; `target/glorp-pixel-active-top.txt`; `target/glorp-pixel-idle-sample.txt`; `target/glorp-pixel-active-sample.txt` | pass | Pixel stayed within both idle and active delta budgets. |
| Resize freshness | `target/task6-artifacts/pixel-260-window.png`; `target/task6-artifacts/pixel-480-window.png`; `target/task6-artifacts/pixel-260-windows.txt`; `target/task6-artifacts/pixel-480-windows.txt` | blocked | Hidden review-size launches prove startup geometry at multiple sizes, but this review did not capture a live resize mutation or next-tick stale-frame check. |
| Privacy | `target/task6-artifacts/privacy-scan-pixel.txt`; `target/task6-logs/dev-preview-test.log`; `target/task6-logs/cargo-test.log` | pass | Saved privacy scan artifact records the exact pattern set and returned `no matches` for seed, source names, paths, prompt/response text, transcripts, or helper diagnostics. The dev-preview privacy test now scans Pixel machine artifacts plus the side-by-side `.txt` and `.cells.json` artifacts. |

## Default Flip Decision

Pixel remains opt-in in this implementation.

Recommendation: blocked by the failed or blocked gates listed above.
