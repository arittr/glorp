# Task 6 Report: Port canonical GPU readback behind production-owned types

## Status: COMPLETE

Landed the canonical GPU readback behind production-owned types, per the
coordinator's two decisions (Option A: `capture(&PairedReviewFrame)` with a
`prepared()` accessor; return `Result<_, RetainedFailureCategory>` with minimal
new variants). All gates pass in both feature configs.

## Files changed

Staged (5):

- `src/companion/retained/capture.rs` (new) — pure normalization + the live wgpu
  readback (`RetainedCaptureTarget::capture`).
- `src/companion/retained.rs` — `mod capture;`, extracted the shared
  `encode_scene` render-pass helper, and pointed `render()` at it.
- `src/companion/paired_review.rs` — added `pub(super) fn prepared(&self) ->
  &PreparedCompanionFrame` (Option A accessor; the frozen `frame` field is no
  longer `#[allow(dead_code)]`).
- `src/companion/retained/presentation.rs` — added 4 `RetainedFailureCategory`
  variants + their static `category()` strings (see "Beyond the 4-file list").
- `tests/retained_renderer_boundary.rs` (new) — source-boundary text scan.

**Beyond the 4-file list — `presentation.rs`:** Decision 2 told me to add the
new error paths as `RetainedFailureCategory` variants. That enum lives in
`src/companion/retained/presentation.rs`, so honoring decision 2 unavoidably
edits that file — the same "legitimate consequence, not scope creep" principle
the coordinator applied to `paired_review.rs`. A compiling commit must include
it, so it is the 5th staged file. It is a retained-module source (the boundary
test's `renderer_spike::` scan already covers `retained/*.rs`, so it is in the
guarded set). No other approach keeps capture on the shared enum that Task 13
extends, which decision 2 explicitly requires.

## TDD evidence (pure normalization)

RED — wrote the two Step-1 tests verbatim in `capture.rs`'s test module with no
implementation and ran `cargo test --features retained-renderer
companion::retained::capture`:

```
error[E0433]: cannot find type `PixelOrder` in this scope
error[E0425]: cannot find function `normalize_readback_rows` in this scope
error[E0433]: cannot find type `PixelOrder` in this scope
error[E0425]: cannot find function `normalize_readback_rows` in this scope
error: could not compile `glorp` (lib test) due to 4 previous errors
```

Exactly the expected missing-symbol failure (brief Step 2).

GREEN — after implementing `PixelOrder`, `aligned_bytes_per_row`,
`normalize_readback_rows`, `CanonicalRgbaFrame`, `ReadbackMetadata`:

```
running 9 tests
test ...::bgra_rows_are_unpadded_swizzled_and_top_left ... ok
test ...::row_normalization_rejects_short_mapped_buffer ... ok
test ...::rgba_rows_pass_through_without_swizzle ... ok
test ...::unpadded_row_is_copied_when_stride_equals_width_times_four ... ok
test ...::short_buffer_is_rejected_with_the_buffer_too_short_category ... ok
test ...::aligned_bytes_per_row_rounds_up_to_the_copy_alignment ... ok
test ...::readback_metadata_describes_the_padded_row_layout ... ok
test ...::rgba_surface_formats_are_not_swizzled ... ok
test ...::capture_failure_categories_are_static_and_hyphenated ... ok
test result: ok. 9 passed; 0 failed
```

Both mandated tests are byte-exact (the BGRA test asserts the drop-pad + B/R
swap + top-left order transform; the short-buffer test asserts rejection). The
added tests harden real transforms (RGBA passthrough, an unpadded row where
`bytes_per_row == width*4`, the aligned-row rounding boundaries, the padded-row
`ReadbackMetadata` layout, and the static error categories) — none test mocked
behavior.

## How the readback was ported (no renderer_spike import)

`RetainedCaptureTarget::capture` re-derives `renderer_spike/wgpu.rs:812-950` from
scratch against production types; nothing is imported from `renderer_spike`
(the boundary test fails the build if it ever is). Sequence:

1. Decompose the frozen frame via `frame.prepared()` + the Task-5 accessors
   (`renderer_source()` matched on `Smooth` for metrics/pet-center/plan/draw
   order; `review_*` for aperture/background/aura/gauges/hud/overlays/dim). A
   non-Smooth variant returns `CaptureUnsupportedVariant` rather than guessing.
2. `ensure_glyph_atlas(collect_glyphs(...))` so the intermediate matches the
   frozen glyph repertoire, then `prepare_gpu_frame(...)`.
3. Physical-size sRGB intermediate texture (`config.format`,
   `RENDER_ATTACHMENT | COPY_SRC`).
4. `copy_texture_to_buffer` into a `COPY_DST | MAP_READ` staging buffer sized by
   `ReadbackMetadata::staging_buffer_size()`, `bytes_per_row =
   aligned_bytes_per_row(width)` (256-byte aligned).
5. A single `queue.submit`, retaining the submission index.
6. `map_async(MapMode::Read, ...)` then `device.poll(PollType::Wait {
   submission_index: Some(idx), timeout: Some(Duration::from_secs(5)) })` — the
   five-second bounded poll.
7. Receive the callback result, `get_mapped_range`, `normalize_readback_rows`,
   `unmap`, and return `CanonicalRgbaFrame`.
8. **Matching IDs (why Option A is safer):** `frame_id`/`resource_generation`
   are stamped from `frame.identity.frame_id` / `.resource_generation` — the
   review row's own IDs — so the "matching IDs" requirement holds structurally
   rather than via a caller-supplied pair.

Channel/row/alpha handling is folded into the pure, tested
`normalize_readback_rows`: it drops the 256-byte row padding, swaps B/R only for
`PixelOrder::Bgra` (derived from the surface format via `pixel_order_for_format`,
covering `Bgra8Unorm`/`Bgra8UnormSrgb`), preserves top-left row order, and
returns `CaptureBufferTooShort` when the mapped buffer is shorter than
`bytes_per_row*(height-1) + width*4`. Alpha is straight sRGB today with a
documented single-spot seam for Task 11's premultiplied-linear convention (the
Step-1 tests use opaque alpha, so any future unpremultiply must stay a no-op
there).

## How the render loop was not duplicated

Extracted `RetainedHost::encode_scene(encoder, target_view, atlas_bind_group,
primitive_buffer, blends, background)` — the clear-loaded render pass +
per-blend `set_pipeline`/`draw(0..6, i..i+1)` loop. `render()` now calls it
targeting the surface view; `capture()` calls the same helper targeting the
intermediate view. There is exactly one render-pass/pipeline loop in the module.
The extraction is borrow-clean: `encode_scene(&self, ...)` shares `self`
immutably alongside the immutable `glyph_atlas`/`device` reborrows while the
`encoder` is a separate `&mut` local.

## New error variants (decision 2)

`RetainedFailureCategory` gained (each with a static `category()` string):
`CaptureUnsupportedVariant` (`retained-capture-unsupported-variant`),
`CapturePollTimeout` (`retained-capture-poll-timeout`), `CaptureMapFailed`
(`retained-capture-map-failed`), `CaptureBufferTooShort`
(`retained-capture-buffer-too-short`). No `GlorpError`/`format!` reaches
`capture.rs`. Task 7 converts a category into a process-level error at its
boundary.

## Feature-gating / dead-code discipline

`capture.rs` compiles only under the retained module's `cfg(all(macos,
retained-renderer))`. `RetainedCaptureTarget` and `capture()` are unused until
Task 7, so they carry a scoped `#[allow(dead_code)]` (no blanket allow);
`CanonicalRgbaFrame` carries one too (its fields are read only by Task 7). Every
pure helper (`normalize_readback_rows`, `aligned_bytes_per_row`,
`ReadbackMetadata` + all its fields, `PixelOrder`, the 4 new error variants) is
exercised by unit tests, so none needs an allow. The boundary test is not
feature-gated (pure `std::fs` text scan) and runs in the default config.

## Verification results

- `cargo test --features retained-renderer companion::retained::capture` — 9
  passed.
- `cargo test --test retained_renderer_boundary` — 2 passed.
- `cargo test --features retained-renderer companion::retained` — 21 passed
  (render refactor did not regress the ladder/atlas/gauge tests).
- `cargo test --features retained-renderer companion::paired_review` — 7 passed
  (accessor did not disturb the checksum/path tests).
- `cargo clippy --lib --features retained-renderer -- -D warnings` — exit 0.
- `cargo clippy --lib -- -D warnings` (feature-off) — exit 0.
- `cargo clippy --all-targets --features retained-renderer -- -D warnings` —
  exit 0 (lints the boundary test target).
- `cargo clippy --all-targets -- -D warnings` (feature-off) — exit 0.
- `cargo build` (feature-off) — exit 0.
- `cargo fmt --check` — exit 0.

## Concerns

- The live wgpu readback (`capture()`) cannot be exercised headlessly here; per
  the brief it is verified live in Task 7/15. This task's gate rests on the pure
  normalization tests + boundary test + both-config compiles, exactly as the
  brief specifies. The GPU sequence is a faithful, line-by-line port of the
  proven spike, but its runtime pixel output is unverified in this task.
- `presentation.rs` is a 5th staged file — the direct, unavoidable consequence
  of decision 2 (variants must live on `RetainedFailureCategory`). Flagged above
  and reflected in the commit's staged set.
- The pre-existing `.superpowers/sdd/task-4-report.md` modification in the tree
  is not mine and was left untouched/unstaged.
