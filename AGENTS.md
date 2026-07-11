# AGENTS.md

Guidance for agentic coding sessions in this repository.

## Preview Lab

Use Glorp's hidden preview command for deterministic design review before
changing the live TUI:

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```

Scenario options:

- `--scenario all` renders watch frames, habitat prop QA frames, the pet
  matrix, round companion frames, and scene animation strips.
- `--scenario watch` renders `watch-wide-normal` at `120x32` and
  `watch-tall-wide` at `180x50`, `watch-compact-normal` at `72x24`, plus
  deterministic liveliness, day-context, room, species-dialect, and activity
  identity fixtures.
- `--scenario props` renders `habitat-props-catalog` plus early, lived-in, and
  full watch frames for prop-density review.
- `--scenario pets` renders `pet-species-stage`, `pet-species-stage-flat`,
  and live-state species fixtures.
- `--scenario round` renders the circular companion previews, including normal,
  activity, asleep, helper-trouble, flat-color, and species-dialect frames.
- `--scenario animation` renders deterministic scene animation strips for
  paused playback review.

The bundle includes `index.html`, `review.md`, `manifest.json`, local assets,
`frames/*.txt` / `frames/*.cells.json` captures, optional
`frames/*.layout.json` captures, optional cropped room artifacts
`frames/*.room.txt` / `frames/*.room-masked.txt`, and typed contract artifacts
such as `frames/*.scene.json`, `frames/*.round-layout.json`, and
`frames/*.round-commands.json`. For animation scenarios the bundle includes
`strips/<id>/frame-NNN.txt` / `strips/<id>/frame-NNN.cells.json` captures.
Treat `manifest.json` as the review contract; it lists scenario intent,
dimensions, files, inputs, typed artifacts, artifact inventory, and review
prompts.
`manifest.json` uses `schema_version` 3. Round scenarios include a `round`
metadata object with target renderer, circular aperture, and privacy claims.
Animation strips are listed in the `strips` array with `kind: "scene-moment"`,
`playback`, `target_id`, and per-frame `phase` / `elapsed_ms` values.

`dev-preview` is intentionally hidden from normal help output. It is a local
development tool and does not read or write real user pet state. Output
replacement is guarded: Glorp only overwrites missing, empty, or previously
owned preview directories marked with `.glorp-preview` and a matching manifest
producer.

Useful checks after preview changes:

```bash
cargo test --features dev-preview --test dev_preview
cargo test --features dev-preview dev_preview::scenarios
cargo test --features dev-preview dev_preview::export
cargo test --features dev-preview dev_preview::habitat_props
cargo test --test round_scene
```

## Dev Task Runner

Use the Rust xtask runner for repo-local development workflows that would
otherwise become shell snippets.

```bash
cargo xtask companion fresh
cargo xtask companion fresh --debug
```

`cargo xtask companion fresh` builds the macOS companion app bundle, quits any
running Glorp companion, waits briefly for macOS to release the process, and
opens the freshly built optimized `target/macos/Glorp.app`. `npm run companion`
delegates to the same command. Use `--debug` only when runtime Objective-C
diagnostics are needed; the debug binary is too expensive for an always-on app.

## Release Procedure

Glorp's full npm release is CI-owned. Do not publish from the repository root:
the root `package.json` is a workspace manifest with no package name, so
root-level `npm publish --access public` is the wrong command. Full
multi-platform publication must run through `.github/workflows/publish.yml`.

Version surfaces must stay in lockstep:

- `Cargo.toml`
- the `glorp` package entry in `Cargo.lock`
- `npm/glorp/package.json`
- `package-lock.json`
- every `npm/platform/*/package.json`
- the platform entries in `npm/glorp`'s `optionalDependencies`

Use the repo helper for release bumps:

```bash
node scripts/bump-npm-version.mjs X.Y.Z
node scripts/assert-release-version.mjs --tag vX.Y.Z
```

The assertion script is the release contract. It must pass before tagging, and
the publish workflow runs it again against `GITHUB_REF_NAME` before any npm
publish step.

Recommended local pre-tag checks:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
npm test
```

Release flow:

1. Sync with `origin/main` before tagging so the tag points at the commit that
   will actually publish.
2. Run the version bump helper and local checks.
3. Commit the release changes.
4. Create an annotated tag: `git tag -a vX.Y.Z -m "Release vX.Y.Z"`.
5. Push the commit and tag.
6. Let `.github/workflows/publish.yml` publish via npm trusted publishing. It
   builds platform binaries, publishes platform packages sequentially, then
   publishes `@arittr/glorp`.
7. After the workflow finishes, verify the npm package/version is visible.

For a no-publish rehearsal, manually run the `publish` workflow with the default
`dry_run` input. This exercises the test/build/smoke matrix and skips the
publish jobs.

## Activity Identity Preview Fixtures

- The `@ccusage/codex` fallback dependency is retained for at least one release
  while the unified `ccusage daily --json` path is validated against real Codex
  usage. Remove it only after provider tests confirm modern `ccusage` emits
  distinct, correctly-normalized Codex rows and the preview ensemble fixture
  renders Codex sources without regressions.
- Preview Lab now includes `watch-activity-identity-ensemble` (four active
  sources) and `watch-activity-identity-unknown` (one unrecognized source).
  Run them with:
  ```bash
  cargo run -- dev-preview --scenario watch --out target/glorp-preview
  open target/glorp-preview/index.html
  ```
