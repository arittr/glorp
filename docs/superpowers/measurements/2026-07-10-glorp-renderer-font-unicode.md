# Glorp Renderer Font And Unicode Qualification

**Date:** 2026-07-10
**Status:** Automated bake-off complete; **human visual policy approval required**
**Decision owner:** repository owner/user

## Scope and method

This bake-off evaluates the bounded current renderer repertoire without reading user state or installing fonts.

The required-glyph manifest was generated from all 269 deterministic Preview Lab `*.cells.json` captures produced by:

```bash
cargo run --features dev-preview -- dev-preview --scenario all \
  --out target/renderer-spikes/wgpu-qualified-font/preview-lab
```

The manifest adds the renderer-decision Unicode oracles explicitly:

- replacement character: `�` (`U+FFFD`)
- non-BMP scalar: `🫧` (`U+1FAE7`)
- multi-scalar key: `o` + combining diaeresis (`U+006F U+0308`)

Result: **172 required keys / 172 Unicode scalars**. See:

- `target/renderer-spikes/wgpu-qualified-font/required-glyph-manifest.json`
- `target/renderer-spikes/wgpu-qualified-font/required-keys.txt`

Coverage was checked with HarfBuzz against only the declared policy font files. AppKit/CoreText then rendered the full repertoire at logical sizes 260, 360, 480, and 720, producing Retina-sized 520, 720, 960, and 1440 PNGs. Metrics record selected font, advance, ascent, descent, leading, and pixel-snapped origin. An initial capture had repeated right-edge clipping; the bake-off tool was corrected to reserve explicit margins and wrap earlier, and all 12 captures were regenerated.

## Candidates and primary licenses

No more than three policies were evaluated.

### 1. Source Code Pro Regular

- Source: Adobe Fonts `source-code-pro`, release branch.
- License artifact: `target/renderer-spikes/wgpu-qualified-font/sources/source-code-pro/LICENSE.md`
- License: SIL Open Font License 1.1.
- Source bytes: **210,312**.
- Required-repertoire subset: **29,936 bytes**.
- Terms: redistribution, embedding, modification, and subsetting are permitted when the OFL and copyright notice accompany the font; modified versions must follow the OFL naming/reserved-name rules and cannot be sold by themselves.

Strict explicit coverage: **151/172**. Missing 21 technical/symbol/oracle keys, including `�` and `🫧`. CoreText review captures display host fallback for those keys, so the visually complete image is not evidence for a self-contained Source Code Pro policy.

**Automated verdict:** reject as the sole policy.

### 2. Noto Sans Mono + Noto Sans Symbols + Noto Sans Symbols 2 + Noto Color Emoji

- Sources: primary Noto font repositories.
- License artifacts:
  - `target/renderer-spikes/wgpu-qualified-font/sources/noto/OFL.txt`
  - `target/renderer-spikes/wgpu-qualified-font/sources/noto/NOTO-EMOJI-OFL.txt`
- License: SIL Open Font License 1.1.
- Source bytes: **12,101,324**.
- Required-repertoire subsets combined: **61,276 bytes**:
  - mono: 30,056
  - symbols: 12,820
  - symbols 2: 5,148
  - emoji: 13,252
- Terms: redistribution, embedding, modification, and subsetting are permitted with the OFL/copyright notices; modified fonts must follow the OFL naming rules and fonts cannot be sold by themselves.

Strict explicit coverage: **172/172, zero missing**.

Assignment counts for the bounded repertoire:

- Noto Sans Mono: 163 keys
- Noto Sans Symbols: 3 keys
- Noto Sans Symbols 2: 5 keys
- Noto Color Emoji: 1 key (`🫧`)

Unicode oracle shaping:

- `�`: Noto Sans Mono glyph 578
- `ö`: Noto Sans Mono composed glyph 184
- `🫧`: Noto Color Emoji glyph 1430

CoreText on this host could not directly instantiate the downloaded Noto Color Emoji file through `CTFontManagerCreateFontDescriptorsFromURL`. The review renderer therefore used host `AppleColorEmoji` for the visual presentation of the one emoji key, while HarfBuzz proved that the declared Noto Color Emoji asset covers it and the deterministic subset is 13,252 bytes. This is a tooling/visual-review limitation, not a coverage claim that Apple Color Emoji may be redistributed.

**Automated recommendation:** select this policy for a future self-contained atlas bake, subject to human acceptance of the captures and a production atlas tool proving it can rasterize the Noto color glyph or explicitly choosing a monochrome project-owned rendering for `🫧`.

### 3. DejaVu Sans Mono

- Source: DejaVu Fonts 2.37 release archive.
- License artifact: `target/renderer-spikes/wgpu-qualified-font/sources/dejavu/LICENSE`
- License: Bitstream Vera-derived permissive font license plus public-domain additions described in the primary license.
- Source bytes: **340,712**.
- Required-repertoire subset: **20,716 bytes**.
- Terms: redistribution and modification are permitted with the copyright/license notices; the names Bitstream/Vera have trademark/naming restrictions, and the font cannot be sold by itself.

Strict explicit coverage: **167/172**. Missing `˄`, `ѱ`, `⁙`, `⌢`, and `🫧`. CoreText review captures use host fallback for missing keys.

**Automated verdict:** reject as the sole policy.

## Size and metrics matrix

All final captures are unclipped and preserve visible padding at every edge:

```text
target/renderer-spikes/wgpu-qualified-font/matrix/
  source-code-pro-{260,360,480,720}.png
  noto-policy-{260,360,480,720}.png
  dejavu-sans-mono-{260,360,480,720}.png
```

Typed evidence:

- `matrix.json` — per-key selected visual font and advance/ascent/descent/leading/pixel snap
- `matrix-summary.json` — size and oracle summary
- `strict-coverage.json` — explicit declared-policy coverage, excluding host fallback
- `subset-sizes.json` — deterministic subset hashes and bytes
- `artifact-sha256.txt` — final capture and JSON hashes

At logical 360 / 12 pt, representative metrics are:

| Policy | Key | Advance | Ascent | Descent | Visual font |
|---|---:|---:|---:|---:|---|
| Source Code Pro | `ö` | 7.20 | 11.81 | 3.28 | SourceCodePro-Regular |
| Noto policy | `ö` | 7.20 | 12.83 | 3.52 | NotoSansMono-Regular |
| Noto policy | `�` | 14.40 | 12.83 | 3.52 | NotoSansMono-Regular |
| DejaVu Sans Mono | `ö` | 7.22 | 11.14 | 2.83 | DejaVuSansMono |

The `🫧` visual review uses host Apple Color Emoji and measures a 16 px advance, 15 px ascent, and 4.69 px descent at this size. The strict coverage artifact separately proves the Noto Color Emoji glyph.

## Visual observations

Automated/agent visual review of the corrected Noto 360 capture found:

- no right-edge or other viewport clipping after the margin correction;
- stable primary text baselines and readable diacritics/descenders;
- zero accidental tofu boxes;
- visible and correct `�`, `ö`, and color `🫧` oracles;
- expected stylistic mismatch between the monochrome Noto repertoire and the host color emoji.

Source Code Pro and DejaVu are legible and well-spaced for their covered repertoire, but their apparently complete CoreText captures depend on undeclared host fallback and therefore do not satisfy the self-contained policy gate.

## Recommendation and unresolved human checkpoint

**Recommendation:** approve the four-font Noto policy for the renderer qualification architecture, with deterministic subsetting to the current repertoire and all OFL notices bundled. Do not commit or ship the downloaded full font files from this evidence root. A later production atlas implementation should retain only reviewed subset outputs and approved license/attribution files.

This is **not yet an approved font decision**. The repository owner/user must explicitly choose one:

1. **Approve Noto policy visual result** using the four corrected captures:
   - `noto-policy-260.png`
   - `noto-policy-360.png`
   - `noto-policy-480.png`
   - `noto-policy-720.png`
2. Reject it and request one bounded visual adjustment.
3. Approve a time-limited native-atlas exception with owner, date, risk, expiry no later than the next renderer-enabled release qualification, and the exact future closure procedure.

Until that approval is recorded, the final backend decision must treat the font gate as **pending**, not passed.
