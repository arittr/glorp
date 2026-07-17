# Task 7 — Preview spatial cues and purposeful motion

## Delivered

- Replaced the retired `round-smooth-motion` preview strip with the deterministic, paused `round-purposeful-locomotion` strip. Its captures cover dwell, a continuous glide, the immediate next turn boundary, and a later depth excursion.
- Added the ten static round spatial-cue fixtures for typed rim, mood, activity, statistics relationship, sleep settling, and wake resumption review. They do not claim to raster native spatial cues.
- Kept preview-only rim disabled as a private typed presentation option. No production companion renderer behavior or `PET_RIM_ENABLED` constant changed; this fixture proves option selection, not rim pixels.
- Redacted the spatial-cue HUD artifacts to fixed `0.5` fills and em-dash text, with exact counts hidden.
- Replaced the raw smooth-motion export contract with a narrow locomotion sidecar: schema version, strip/segment identifiers, phase, rounded segment phase, public planar/depth buckets, and facing only.

The task brief still names manifest schema version 3. The repository's established preview manifest schema is version 8, and this work deliberately preserves version 8.

## Test-first evidence

- The initial focused tests failed because `round-purposeful-locomotion` and the spatial rim fixtures did not exist.
- Follow-up assertions exposed two selection defects: the dwell capture was not at a real motion boundary, and the turn capture skipped the immediate next boundary. The strip now aligns its fixed review start to a production segment boundary and records that immediate boundary.
- The final truthfulness regression asserts each spatial fixture classifies its actual evidence, keeps native HUD projection and rim pixels out of Preview Lab claims, and directs native-pixel review to retained/AppKit evidence or final local companion QA.
- The completed focused suite passed:
  - `cargo test --features dev-preview --test dev_preview` — 82 tests
  - `cargo test --features dev-preview dev_preview::scenarios` — 6 tests
  - `cargo test --features dev-preview dev_preview::export` — 15 tests

These commands retain pre-existing Rust compiler warnings in unrelated companion-scene/app code.

## Export and privacy verification

- Ran `cargo run -- dev-preview --scenario animation --out target/glorp-preview` and inspected the locomotion sidecars. They contain the required paused phases: `dwell-start`, `dwell-end`, `glide-quarter`, `glide-half`, `glide-three-quarters`, `glide-end`, `turn-boundary`, and `depth-excursion`.
- Re-ran `cargo run -- dev-preview --scenario round --out <temporary-owned-preview-dir>`; the generated manifest reports schema version 8, contains every spatial fixture, and labels native-pixel boundaries correctly.
- Scanned the new spatial HUD artifacts for the former exact count strings and implementation leakage; none was present.
- Reviewed the generated preview bundle in the browser. The round cells retain normal companion art, and the actual cell captures remain useful for static pose and the purposeful-locomotion strip.

Preview Lab does not raster native HUD projection or rim pixels. Statistics frames export typed Smooth plans plus transformed cells and a redacted HUD sidecar; rim fixtures export a typed presentation-option contract. The manifest labels those boundaries explicitly, and the review prompts send visual native spatial-cue verification to native retained/AppKit test evidence or final local companion visual QA. No raster effects or private coverage data are added to the export.

## Changed surfaces

- `src/dev_preview/contract.rs`
- `src/dev_preview/export.rs`
- `src/dev_preview/pixel.rs`
- `src/dev_preview/round.rs`
- `src/dev_preview/scenarios.rs`
- `src/dev_preview/smooth.rs`
- `src/dev_preview/strips.rs`
- `tests/dev_preview.rs`
