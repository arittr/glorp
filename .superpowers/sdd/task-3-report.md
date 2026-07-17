# Task 3: integrate calm lifecycle locomotion

## Outcome

The round companion's wandering path now comes from the deterministic shared
locomotion sampler. Both round-scene builders construct the same lifecycle
input from the pet seed and flattened wake-resume fields, so planar position,
depth intent, and facing originate from one sample.

Sleep settles the sampled pose to neutral with a minimum-jerk curve and then
holds it. A valid wake interval eases from that held neutral pose back to the
live route. Incomplete, inverted, or future wake data asserts in test/debug
builds and falls back to live motion in release builds.

Activity/calm inputs no longer scale locomotion geometry, and the companion
body bob is always zero. The existing legacy smooth-renderer entry point is a
zero-valued compatibility shim only; it retains no sinusoidal behavior and can
be removed when that renderer adopts the shared projection contract.

`FrameSnapshot.reduce_motion` is renderer-private and explicitly skipped from
serialization, checksums, and `Debug`. Reduce Motion still freezes the
presentation transition/parallax behavior without altering the shared
locomotion placement contract.

## RED evidence

Before the implementation, the new `round::motion` tests failed because:

- activity/calm produced different awake planar offsets;
- `round_companion_bob(250)` returned `0.23334524` rather than zero.

The cross-path test initially failed to compile because
`CompanionPetPlacement` did not expose its already-computed motion projection.
The implementation adds a read-only accessor solely for parity verification;
it does not select or split renderers.

## GREEN evidence

The final focused checks all passed:

```text
cargo test --lib round::locomotion::tests                         15 passed
cargo test --lib round::motion::tests                              9 passed
cargo test --lib round::placement::tests                           9 passed
cargo test --lib presentation::companion_scene::input::tests      39 passed
cargo test --test round_scene                                     8 passed
cargo fmt --check                                                  passed
git diff --check                                                   passed
cargo clippy --lib --bins --features retained-renderer -- -D warnings
                                                                  passed
```

## Preview Lab controller follow-up

The three `dev_preview::smooth` failures reported above as unrelated were
Task 3 regressions. Wander mode now samples Task 2's identity- and
wall-clock-driven locomotion route, but Preview Lab was still using a
22-second oscillator search and a `drift_period_secs` mutation that wander
mode no longer reads.

### RED evidence

Before this repair, `cargo test --features dev-preview dev_preview::smooth
-- --nocapture` reproduced all three failures:

- `pinned_reviewed_motion_start_satisfies_preview_contract`;
- `smooth_motion_start_now_prefers_reviewed_start_when_it_passes_contract`;
- `smooth_motion_start_now_returns_first_passing_aligned_search_offset`.

The stale pin failed the current route contract, the fallback selected the old
pin plus 6,720 ms, and the period-mutation fixture could not affect the
wander route. Expanding the bounded search also demonstrated that an
unbuildable Smooth candidate must count as contract failure rather than panic.

### GREEN evidence

The repaired controller re-pins the default route window, searches within
Task 2's 60-second locomotion segment, and uses a deterministic route
fixture (`preview-route-search-fixture`) whose first valid aligned window is
3,680 ms. Candidate windows that cannot build a Smooth plan are rejected.
This remains Preview Lab contract logic; it does not change renderer or public
companion behavior.

```text
cargo test --features dev-preview dev_preview::smooth -- --nocapture    4 passed
cargo fmt --check                                                        passed
git diff --check                                                         passed
```

`cargo test --features dev-preview --test dev_preview` ran 80 tests: 78
passed and two pre-existing, out-of-scope role-count assertions failed. Both
expect 20 canonical roles while the unchanged output includes the existing
fixed `prop-shadows` layer as role 21:

- `dev_preview_smooth_motion_sidecars_show_fractional_progression_and_all_bundle_includes_them`;
- `dev_preview_smooth_sidecars_are_sanitized_and_report_parity`.

`cargo test` passed all 1,295 library tests, including the repaired smooth
tests, before stopping on the pre-existing out-of-scope source-shape assertion
`ui_tick_owns_preparation_and_smooth_uses_the_fallible_planner` in
`tests/companion_draw_boundary.rs`. That assertion already exists in
`ff391a9` and targets unchanged companion-app code.

## Files changed

- `src/round/motion.rs`: lifecycle sampling, neutral settle/wake blend,
  zero bob, and focused motion tests.
- `src/round/scene.rs`, `src/round/placement.rs`: stable-seed/lifecycle input
  and round projection parity observation surface.
- `src/presentation/companion_scene/{input.rs,mod.rs,runtime.rs,scene.rs}`:
  matching input construction plus private Reduce Motion handling and privacy
  assertions.
- `src/presentation/smooth.rs`: updates the stale bob expectation to the
  static contract.
- `tests/round_scene.rs`: round-builder versus canonical-scene projection
  parity integration coverage.
- `src/dev_preview/smooth.rs`: Task 2 route-timed Preview Lab search, a
  re-pinned default window, safe rejection of unbuildable candidates, and a
  deterministic route-search fixture.
- `src/round/scene.rs`: corrects the stale comment so
  `drift_period_secs` is explicitly legacy non-wander behavior.

## Self-review and commit provenance

The review verified that depth overrides affect only depth, the non-wander
Classic surface remains on its legacy placement path, and no activity/calm
geometry or body bob remains. The task began from base commit
`d6dd3b9`; the resulting commit SHA is reported by the handoff because a
commit cannot contain its own content-addressed SHA.
