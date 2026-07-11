# Glorp Retained 360 Parity — Accepted

**Gate:** Task 15 (live 360 Smooth/Retained parity) of the retained-companion default-cutover plan.
**Date:** 2026-07-11
**Verdict:** **ACCEPTED by Drew** on the live Apple-Silicon hardware, after a focused fix loop.
**Branch:** `retained-companion-cutover`
**Commit at acceptance:** `eec3692`

## Commands

```bash
# Live parity oracle: frozen from the actual current companion (S6), not a fixture.
cargo xtask companion review-pair --size 360 --out target/glorp-review/live-360
# Redacted confirmatory capture at the accepted commit:
cargo xtask companion review-pair --size 360 --state normal --out target/glorp-review/gamma-check
# Live side-by-side judged on-device: `companion-app --renderer retained` vs `--renderer smooth`.
```

## Accepted evidence (redacted capture, `gamma-check`)

- status: `success`; requested = effective = `retained`; `last_fallback_reason`: none.
- compiled capabilities include `retained-renderer`.
- frame checksum: `3a9d6c2a9ad58487dc23c44ade10617fd2772f2b38ed7e6dd350d54ec43e8d63` (Smooth + Retained sections share it — same frozen frame).
- logical 360×360 → physical 720×720 at backing scale 2.0 (physical = logical × scale).
- Retained milestones include `gpu-completed` + `readback-completed`; terminal disposition `captured`.
- Smooth terminal disposition `smooth-painted`.
- privacy mode: `redacted` (no HUD text or live values recorded here).
- Zero post-activation atlas churn; no fallback; non-sRGB (`Bgra8Unorm`) live surface confirmed working on hardware.

## What the live gate found and fixed (rejection loop, plan Task-15 Step 3)

The visual gate on real hardware surfaced fidelity differences the redacted automated captures could not (their HUD has no digits/units, and Mac captures had mis-tagged the Smooth reference's compositing color space). Each was fixed with a failing test in the owning area, then re-confirmed live:

- `6f448b2`, `57da373` — glyph repertoire completeness (bold weight; redacted-HUD charset): a native run caught a fallback + hang the headless suite missed.
- `562521c` — **glyph baseline**: `glyph_ink_rect` inverted each glyph's vertical bearing, floating low-ink glyphs (the decimal `.` and lowercase unit letters) upward. One fix corrected both the raised decimal and the `yday`/`10m` "superscript" look.
- `7014723` — **cast shadow**: the floor-projection radial gradient was mapped to the opaque tank primitive, dropping its falloff (flat disc). Fixed with a dedicated premultiplied radial gradient with edge fade.
- `eec3692` — **translucency compositing: gamma (Decision 1 resolved)**. Drew accepted switching from the plan's physically-correct premultiplied-**linear** convention to premultiplied-**gamma** (sRGB-space) to match how CoreGraphics/Smooth — and the live display — actually composite. This darkened the floor and settled the unfilled gauge tracks into parity. It also completed the compositing-space half of the Smooth-capture-fidelity fix (the capture reference now composites in the same sRGB space as the live view). Opaque colors stay Δ0; the swatch oracle's worst translucent delta dropped from Δ43 to Δ1.

Decision-1 note: this deliberately overrides the plan's §2/§5 "premultiplied linear" mandate. The plan's goal is visual parity with the shipping Smooth companion, and Smooth composites in gamma; the plan itself pre-marked this as the fallback if the translucency difference was rejected.

## Reversibility

The whole cutover remains gated behind the single constant `AUTO_RETAINED_ON_APPLE_SILICON` (still `false` at this commit — Auto resolves to Smooth). Smooth stays compiled, explicitly selectable, and the automatic fallback.
