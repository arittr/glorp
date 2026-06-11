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

- `--scenario all` renders watch frames, the habitat prop QA frames, the pet
  matrix, and scene animation strips.
- `--scenario watch` renders `watch-wide-normal` at `120x32` and
  `watch-tall-wide` at `180x50`, and `watch-compact-normal` at `72x24`.
- `--scenario props` renders `habitat-props-catalog` plus early, lived-in, and
  full watch frames for prop-density review.
- `--scenario pets` renders `pet-species-stage`, covering all six species
  across all seven growth stages.
- `--scenario animation` renders scene animation strips (currently
  `scene-strip-smoke`) for paused playback review.

The bundle includes `index.html`, `review.md`, `manifest.json`, local assets,
`frames/*.txt` / `frames/*.cells.json` captures, and for animation scenarios
`strips/<id>/frame-NNN.txt` / `strips/<id>/frame-NNN.cells.json` captures.
Treat `manifest.json` as the review contract; it lists scenario intent,
dimensions, files, inputs, and review prompts. `manifest.json` uses
`schema_version` 2 and includes a `strips` array whose entries have
`kind: "scene-moment"` along with `playback`, `target_id`, and per-frame
`phase` / `elapsed_ms` values.

`dev-preview` is intentionally hidden from normal help output. It is a local
development tool and does not read or write real user pet state. Output
replacement is guarded: Glorp only overwrites missing, empty, or previously
owned preview directories marked with `.glorp-preview` and a matching manifest
producer.

Useful checks after preview changes:

```bash
cargo test --test dev_preview
cargo test dev_preview::scenarios
cargo test dev_preview::export
cargo test dev_preview::habitat_props
```

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
