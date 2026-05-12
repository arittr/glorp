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

- `--scenario all` renders both watch frames and the pet matrix.
- `--scenario watch` renders `watch-wide-normal` at `120x32` and
  `watch-compact-normal` at `72x24`.
- `--scenario pets` renders `pet-species-stage`, covering all six species
  across all seven growth stages.

The bundle includes `index.html`, `review.md`, `manifest.json`, local assets,
and `frames/*.txt` / `frames/*.cells.json` captures. Treat `manifest.json` as
the review contract; it lists scenario intent, dimensions, files, inputs, and
review prompts.

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
```
