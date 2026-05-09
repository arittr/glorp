# Glorp Rust Terminal Pet Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the production Rust `glorp` CLI/TUI so every story in `docs/superpowers/stories/` is complete and every acceptance criterion is met.

**Architecture:** Create one Rust binary with clean boundaries: CLI commands, local storage, usage providers, pet/game rules, deterministic renderer, and Ratatui watch mode. Usage ingestion shells out to `ccusage` and `ccusage-codex`, stores only normalized usage metadata, and feeds the pet through calibrated effective-token deltas. The TUI should carry over the full `docs/tokenpet/project/tokenpet.html` visual system -- colors, type rhythm, chrome, borders, log treatments, bars, overlays, animation feel, and compact behavior -- while removing prototype-only controls.

**Tech Stack:** Rust 2021, `clap`, `serde`, `rusqlite`, `ratatui`, `crossterm`, `assert_cmd`, `insta`, npm JavaScript launcher, bundled `ccusage` and `@ccusage/codex` helpers.

---

## Source Material

- Product spec: `docs/superpowers/specs/2026-05-08-glorp-design.md`
- Story cards: `docs/superpowers/stories/*.md`
- Mockup guidance: `docs/tokenpet/README.md`, `docs/tokenpet/chats/chat1.md`, `docs/tokenpet/project/tokenpet.html`, `docs/tokenpet/project/app.jsx`, `docs/tokenpet/project/pet.jsx`, `docs/tokenpet/project/extras.jsx`
- Current verified helper facts on 2026-05-09:
  - Local `ccusage --version` reports `18.0.10`; `ccusage daily --help` supports `daily --json --offline --order asc`.
  - Local `ccusage-codex` is not on PATH in this checkout, but `npm exec --yes @ccusage/codex@18.0.11 -- --version` reports `18.0.11`, and `daily --help` supports `daily --json --offline` with no `--order` flag.
  - `ccusage daily --json --offline --order asc` returns root keys `daily` and `totals`. Row keys include `date`, `inputTokens`, `outputTokens`, `cacheCreationTokens`, `cacheReadTokens`, `totalTokens`, `totalCost`, `modelsUsed`, and `modelBreakdowns`. Each `modelBreakdowns` row includes `modelName`, token buckets, and `cost`.
  - `@ccusage/codex@18.0.11 daily --json --offline` returns root keys `daily` and `totals`. Row keys include `date`, `inputTokens`, `outputTokens`, `cachedInputTokens`, `reasoningOutputTokens`, `totalTokens`, `costUSD`, and `models`; `models` is an object keyed by model name.

## Prototype Translation Rules

Carry into Rust:

- The full Tokenpet terminal style, not just the layout: warm-black background, dark surface, parchment foreground, muted dim/faint text, amber accent, moss-green positive state, coral error/hunger state, restrained window chrome, dashed section rules, block vitals bars, muted log timestamps, colored log/event left rails, help overlay, sparkline-like recent activity, and bell/evolution flash.
- Six species: `fuzz`, `blob`, `ghost`, `glitch`, `crystal`, `mech`.
- Seeded generation for species, visible traits, palette, name, morph, and animation phase.
- Seven species-specific stages, calibrated evolution, subtle independent breathing/blinking/species animation, mood expressions, and a recoverable wilted floor state.
- `glorp` naming, CLI-first mental model, and a short hatching moment during `glorp init`.

Remove from production:

- Manual food commands, fake token buttons, `ship` mechanics, PR/commit/diff stats, treats, tweak panel, stage override, species override, litter picker, graveyard, revive, death/permadeath, and fake install streams.
- Any prompt text, response text, tool-call payload, copied transcript content, or source transcript archival behavior.

## Tokenpet Style Contract

The Rust app should treat the Tokenpet mockup as the source of truth for visual style. If terminal constraints require adaptation, adapt by reducing fidelity, not by picking a new aesthetic.

Canonical palette from `docs/tokenpet/project/tokenpet.html`:

| Token | Source OKLCH | Terminal RGB Approx | Usage |
| --- | --- | --- | --- |
| `bg` | `oklch(0.18 0.005 60)` | `#13110f` | main terminal background |
| `surface` | `oklch(0.22 0.006 60)` | `#1d1a18` | chrome, panels, overlays |
| `fg` | `oklch(0.94 0.01 80)` | `#efebe4` | primary text and pet body fallback |
| `dim` | `oklch(0.66 0.012 70)` | `#97918a` | labels, normal log text, secondary metadata |
| `faint` | `oklch(0.42 0.008 60)` | `#504c49` | section rules, separators, timestamps, empty bars |
| `accent` | `oklch(0.78 0.14 70)` | `#f0a646` | amber highlights, prompt path, stage, XP, evolution, help title |
| `good` | `oklch(0.74 0.10 145)` | `#82bc83` | provider healthy, real food/usage gained, positive deltas |
| `bad` | `oklch(0.68 0.16 25)` | `#ea6a64` | provider errors, hunger/sad/wilted warnings |

Component style carryover:

- Window/chrome: preserve the mock terminal title bar feel with small red/yellow/green dots where width allows, a dim centered title, and a darker `surface` strip over `bg`.
- Layout: side-by-side columns at wide widths, vertical compact mode under roughly 72 columns, no card-heavy dashboard look.
- Typography: terminal monospace only; use compact labels and tabular numbers. Ratatui cannot force JetBrains Mono, but spacing should match the mockup's dense CLI rhythm.
- Borders/rules: use restrained single-line borders and dashed/faint section dividers; avoid rounded-card/product-dashboard styling.
- Vitals: use 20-cell unicode block bars with filled `█` and empty `░`, matching Tokenpet's gamey terminal bars.
- Feed/event log: timestamps are `faint`, normal text is `dim`, real usage/feed events are `good`, evolution/help/accent events are `accent`, diagnostics are `bad`, and important event rows get a subtle left rail when the terminal width allows.
- Pet color: seeded body/eye/mouth/accent/pattern colors should follow the mockup's hue-shift model, but stay restrained; do not introduce neon or rainbow palettes.
- Mood colors: happy/content lean `good`/`accent`, hungry/sad/sleepy lean `bad`/`dim`, wilted is dimmed/desaturated in terminal terms through `faint`/`dim` glyph choices and drooped art.
- Overlays/help: help and evolution overlays should use `surface`/`bg` with `accent` borders and titles, not a separate design language.
- Animations: keep independent, subtle rhythms from the mockup: breathing, blinking, species flavor, and event flashes should run on separate seeded phases and should not feel synchronized or frantic.

## File Structure

Create this structure:

```text
Cargo.toml
.gitignore
README.md
src/
  main.rs
  lib.rs
  cli.rs
  config.rs
  error.rs
  paths.rs
  time.rs
  commands/
    mod.rs
    init.rs
    reset.rs
    status.rs
    doctor.rs
    watch.rs
  game/
    mod.rs
    calibration.rs
    effective_tokens.rs
    evolution.rs
    metabolism.rs
  pet/
    mod.rs
    art.rs
    generation.rs
    render.rs
  storage/
    mod.rs
    state.rs
    usage_store.rs
  tui/
    mod.rs
    app.rs
    layout.rs
    style.rs
    view_model.rs
  usage/
    mod.rs
    ccusage.rs
    normalize.rs
    provider.rs
tests/
  cli_smoke.rs
  doctor_status.rs
  generation.rs
  storage_privacy.rs
  usage_provider.rs
  game_rules.rs
  style_tokens.rs
  tui_render.rs
  fixtures/
    helpers/
      ccusage-ok.mjs
      ccusage-codex-ok.mjs
      ccusage-fails.mjs
      ccusage-prompts.mjs
      ccusage-invalid-json.mjs
      ccusage-secret-stderr.mjs
    ccusage-daily.json
    ccusage-daily-next.json
    ccusage-codex-daily.json
npm/
  glorp/
    package.json
    bin/glorp.js
    test/smoke.mjs
  platform/
    darwin-arm64/package.json
    darwin-x64/package.json
    linux-x64/package.json
    linux-arm64/package.json
    win32-x64/package.json
scripts/
  build-platform-package.mjs
docs/superpowers/build-report.yaml
```

## Task 1: Bootstrap Rust CLI And Test Harness

**Stories:** foundation for all stories.

**Files:**
- Create: `Cargo.toml`
- Create: `.gitignore`
- Create: `src/main.rs`
- Create: `src/lib.rs`
- Create: `src/cli.rs`
- Create: `src/error.rs`
- Create: `tests/cli_smoke.rs`

- [ ] **Step 1: Create the Rust package manifest**

Use this initial `Cargo.toml`:

```toml
[package]
name = "glorp"
version = "0.1.0"
edition = "2021"
license = "MIT"

[[bin]]
name = "glorp"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive", "env"] }
crossterm = "0.28"
ratatui = "0.29"
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
time = { version = "0.3", features = ["serde", "formatting", "parsing", "local-offset", "macros"] }
toml = "0.8"
which = "7"

[dev-dependencies]
assert_cmd = "2"
insta = { version = "1", features = ["yaml"] }
predicates = "3"
tempfile = "3"
```

- [ ] **Step 2: Create the CLI argument model**

Add `src/cli.rs`:

```rust
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "glorp", version, about = "A terminal pet fed by real AI coding token usage")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create local state and hatch the first pet.
    Init {
        #[arg(long)]
        seed: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    /// Run the live terminal pet beside your coding session.
    Watch,
    /// Print a compact non-interactive pet and usage summary.
    Status,
    /// Rename the current pet without changing its seed-derived traits.
    Rename { name: String },
    /// Confirmed full reset of Glorp pet state.
    Reset {
        #[arg(long)]
        yes: bool,
    },
    /// Inspect helper availability, config paths, parser health, and diagnostics.
    Doctor,
    /// Show command help.
    Help,
}
```

- [ ] **Step 3: Wire the binary entrypoint**

Add `src/error.rs`:

```rust
use thiserror::Error;

pub type Result<T> = std::result::Result<T, GlorpError>;

#[derive(Debug, Error)]
pub enum GlorpError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
}
```

Add `src/lib.rs`:

```rust
pub mod cli;
pub mod error;

use clap::Parser;
use cli::{Cli, Command};
use error::Result;

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Help => {
            Cli::command().print_help()?;
            println!();
        }
        other => {
            println!("glorp command parsed: {other:?}");
        }
    }
    Ok(())
}
```

Add `src/main.rs`:

```rust
fn main() {
    if let Err(err) = glorp::run() {
        eprintln!("glorp: {err}");
        std::process::exit(1);
    }
}
```

- [ ] **Step 4: Write CLI smoke tests**

Add `tests/cli_smoke.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_lists_mvp_commands_and_no_manual_feed() {
    let mut cmd = Command::cargo_bin("glorp").unwrap();
    cmd.arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("watch"))
        .stdout(predicate::str::contains("status"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("reset"))
        .stdout(predicate::str::contains("rename"))
        .stdout(predicate::str::contains("feed").not());
}
```

- [ ] **Step 5: Run the bootstrap tests**

Run:

```bash
cargo test --test cli_smoke
```

Expected: the test fails until `Help` prints real Clap help for all commands and no manual `feed` command exists.

- [ ] **Step 6: Make the help path pass**

Update `src/lib.rs` imports so `Cli::command()` compiles:

```rust
use clap::{CommandFactory, Parser};
```

Run:

```bash
cargo test --test cli_smoke
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml .gitignore src/main.rs src/lib.rs src/cli.rs src/error.rs tests/cli_smoke.rs
git commit -m "chore: bootstrap glorp rust cli"
```

## Task 2: Local Paths, State, SQLite, And Privacy Guardrails

**Stories:** `story-002`

**Files:**
- Create: `src/config.rs`
- Create: `src/paths.rs`
- Create: `src/storage/mod.rs`
- Create: `src/storage/state.rs`
- Create: `src/storage/usage_store.rs`
- Create: `tests/storage_privacy.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing storage and privacy tests**

Add `tests/storage_privacy.rs`:

```rust
use glorp::paths::AppPaths;
use glorp::config::AppConfig;
use glorp::storage::state::{PetState, StateStore, Vitals};
use glorp::storage::usage_store::{NormalizedUsageEvent, UsageStore};
use tempfile::tempdir;
use time::{Duration, OffsetDateTime};

#[test]
fn state_files_stay_inside_config_override() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    assert!(paths.config_file.starts_with(dir.path()));
    assert!(paths.state_file.starts_with(dir.path()));
    assert!(paths.usage_db.starts_with(dir.path()));
}

#[test]
fn default_paths_are_under_home_config_glorp() {
    let home = tempdir().unwrap();
    let paths = AppPaths::from_home_dir(home.path().to_path_buf());
    assert_eq!(paths.config_dir, home.path().join(".config/glorp"));
    assert_eq!(paths.config_file, home.path().join(".config/glorp/config.toml"));
    assert_eq!(paths.state_file, home.path().join(".config/glorp/state.json"));
    assert_eq!(paths.usage_db, home.path().join(".config/glorp/usage.sqlite"));
}

#[test]
fn pet_state_round_trips_schema_and_vitals() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    let store = StateStore::new(paths.state_file.clone());
    let state = PetState::new_for_test("mochi-7f3a", "mochi");
    store.save(&state).unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.schema_version, 1);
    assert_eq!(loaded.pet.seed, "mochi-7f3a");
    assert_eq!(loaded.pet.accepted_name, "mochi");
    assert_eq!(loaded.vitals, Vitals { fed: 70.0, happiness: 70.0, energy: 70.0 });
}

#[test]
fn malformed_or_unsupported_state_returns_error_without_resetting() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    let store = StateStore::new(paths.state_file.clone());

    std::fs::write(&paths.state_file, "{not valid json").unwrap();
    let malformed = store.load().unwrap_err().to_string();
    assert!(malformed.contains("malformed") || malformed.contains("JSON"));
    assert!(paths.state_file.exists());

    std::fs::write(&paths.state_file, r#"{"schema_version":999}"#).unwrap();
    let unsupported = store.load().unwrap_err().to_string();
    assert!(unsupported.contains("unsupported schema version"));
    assert!(paths.state_file.exists());
}

#[test]
fn config_defaults_and_cache_read_weight_override_load() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    let default_config = AppConfig::load_or_default(&paths.config_file).unwrap();
    assert_eq!(default_config.cache_read_weight, 0.03);

    std::fs::create_dir_all(&paths.config_dir).unwrap();
    std::fs::write(&paths.config_file, "cache_read_weight = 0.05\n").unwrap();
    let overridden = AppConfig::load_or_default(&paths.config_file).unwrap();
    assert_eq!(overridden.cache_read_weight, 0.05);
}

#[test]
fn normalized_usage_storage_never_persists_transcript_payloads() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    let mut store = UsageStore::open(&paths.usage_db).unwrap();
    let event = NormalizedUsageEvent::for_test_with_ignored_payloads(
        "claude-code",
        "prompt text must not persist",
        "response text must not persist",
        "tool payload must not persist",
    );
    store.insert_event(&event).unwrap();
    let raw_db = std::fs::read(&paths.usage_db).unwrap();
    let text = String::from_utf8_lossy(&raw_db);
    assert!(!text.contains("prompt text must not persist"));
    assert!(!text.contains("response text must not persist"));
    assert!(!text.contains("tool payload must not persist"));
}

#[test]
fn compacts_events_older_than_ninety_days_without_losing_lifetime_counters() {
    let dir = tempdir().unwrap();
    let paths = AppPaths::from_config_dir(dir.path().to_path_buf());
    let mut store = UsageStore::open(&paths.usage_db).unwrap();
    let now = OffsetDateTime::parse("2026-05-09T12:00:00Z", &time::format_description::well_known::Rfc3339).unwrap();
    store.insert_event(&NormalizedUsageEvent::for_test_at(now - Duration::days(91), 1000.0)).unwrap();
    store.insert_event(&NormalizedUsageEvent::for_test_at(now, 250.0)).unwrap();
    store.compact_before(now - Duration::days(90)).unwrap();
    assert_eq!(store.recent_event_count().unwrap(), 1);
    assert_eq!(store.daily_aggregate_effective_tokens("claude-code").unwrap(), 1000.0);
    assert_eq!(store.lifetime_effective_tokens().unwrap(), 1250.0);
}
```

- [ ] **Step 2: Run tests to confirm missing modules fail**

Run:

```bash
cargo test --test storage_privacy
```

Expected: FAIL with unresolved imports for `config`, `paths`, and `storage`.

- [ ] **Step 3: Implement app paths**

Add `src/paths.rs`:

```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub state_file: PathBuf,
    pub usage_db: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> crate::error::Result<Self> {
        if let Some(dir) = std::env::var_os("GLORP_CONFIG_DIR") {
            return Ok(Self::from_config_dir(PathBuf::from(dir)));
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| crate::error::GlorpError::Message("HOME is not set; set GLORP_CONFIG_DIR to choose a config directory".into()))?;
        Ok(Self::from_home_dir(home))
    }

    pub fn from_home_dir(home: PathBuf) -> Self {
        Self::from_config_dir(home.join(".config").join("glorp"))
    }

    pub fn from_config_dir(config_dir: PathBuf) -> Self {
        Self {
            config_file: config_dir.join("config.toml"),
            state_file: config_dir.join("state.json"),
            usage_db: config_dir.join("usage.sqlite"),
            config_dir,
        }
    }

    pub fn ensure(&self) -> crate::error::Result<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        Ok(())
    }
}
```

- [ ] **Step 4: Implement config loading**

Add `src/config.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_cache_read_weight")]
    pub cache_read_weight: f64,
}

fn default_cache_read_weight() -> f64 {
    0.03
}

impl Default for AppConfig {
    fn default() -> Self {
        Self { cache_read_weight: default_cache_read_weight() }
    }
}

impl AppConfig {
    pub fn load_or_default(path: &Path) -> crate::error::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&text)
            .map_err(|err| crate::error::GlorpError::Message(format!("malformed config.toml: {err}")))?;
        if !(0.0..=1.0).contains(&config.cache_read_weight) {
            return Err(crate::error::GlorpError::Message(
                "cache_read_weight must be between 0.0 and 1.0".into(),
            ));
        }
        Ok(config)
    }
}
```

- [ ] **Step 5: Implement state JSON**

Add `src/storage/mod.rs`:

```rust
pub mod state;
pub mod usage_store;
```

Add `src/storage/state.rs` with `PetState`, `PetIdentity`, `Vitals`, and `StateStore`. Store schema version, seed, generated species, accepted name, timestamps, current stage, XP, vitals, and last update metadata. Malformed JSON and unsupported schema versions must return actionable errors and must not overwrite the file.
Include `PetState::new_for_test(seed, name)` as a small public constructor because integration tests compile as external crates and cannot use `#[cfg(test)]`-only internals.

Core definitions:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PetState {
    pub schema_version: u32,
    pub pet: PetIdentity,
    pub stage: String,
    pub xp: f64,
    pub vitals: Vitals,
    pub created_at: OffsetDateTime,
    pub last_updated_at: OffsetDateTime,
    pub last_usage_poll_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PetIdentity {
    pub seed: String,
    pub generated_species: String,
    pub accepted_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vitals {
    pub fed: f64,
    pub happiness: f64,
    pub energy: f64,
}

pub struct StateStore {
    path: PathBuf,
}
```

- [ ] **Step 6: Implement SQLite usage storage**

Add `src/storage/usage_store.rs`. Use tables named `provider_cursors`, `usage_events`, `daily_aggregates`, `provider_diagnostics`, and `lifetime_counters`. Store normalized token buckets, effective tokens, optional local-derived cost display metadata, confidence label, command/version, source surface, period/timestamp, and model when available.

The schema must not include columns for raw prompt, raw response, tool-call payload, copied transcript content, or transcript archives.
Include public test fixture constructors `NormalizedUsageEvent::for_test_at(...)` and `NormalizedUsageEvent::for_test_with_ignored_payloads(...)` so integration tests exercise the same storage API without gaining access to raw transcript fields.

- [ ] **Step 7: Export modules**

Modify `src/lib.rs`:

```rust
pub mod config;
pub mod paths;
pub mod storage;
```

- [ ] **Step 8: Run storage tests**

Run:

```bash
cargo test --test storage_privacy
```

Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src/config.rs src/paths.rs src/storage tests/storage_privacy.rs src/lib.rs
git commit -m "feat: add local state and usage persistence"
```

## Task 3: ccusage Provider Boundary And Cursor Diffing

**Stories:** `story-001`

**Files:**
- Create: `src/usage/mod.rs`
- Create: `src/usage/provider.rs`
- Create: `src/usage/normalize.rs`
- Create: `src/usage/ccusage.rs`
- Create: `tests/usage_provider.rs`
- Create: `tests/fixtures/helpers/ccusage-ok.mjs`
- Create: `tests/fixtures/helpers/ccusage-codex-ok.mjs`
- Create: `tests/fixtures/helpers/ccusage-fails.mjs`
- Create: `tests/fixtures/helpers/ccusage-prompts.mjs`
- Create: `tests/fixtures/helpers/ccusage-invalid-json.mjs`
- Create: `tests/fixtures/helpers/ccusage-secret-stderr.mjs`
- Create: `tests/fixtures/ccusage-daily.json`
- Create: `tests/fixtures/ccusage-daily-next.json`
- Create: `tests/fixtures/ccusage-codex-daily.json`
- Modify: `src/lib.rs`

- [ ] **Step 1: Add cross-platform fixture helper scripts**

Use Node fixture helpers instead of Unix-only shell scripts so provider tests remain compatible with the Windows package target.

`tests/fixtures/helpers/ccusage-ok.mjs`:

```javascript
#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
if (args[0] === "--version") {
  console.log("ccusage 18.0.11");
  process.exit(0);
}
if (args[0] === "daily" && args.includes("--json") && args.includes("--offline")) {
  const file = process.env.CCUSAGE_FIXTURE ?? "ccusage-daily.json";
  process.stdout.write(fs.readFileSync(path.join(here, "..", file), "utf8"));
  process.exit(0);
}
console.error(`unsupported ccusage fixture args: ${args.join(" ")}`);
process.exit(2);
```

`tests/fixtures/helpers/ccusage-codex-ok.mjs`:

```javascript
#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
if (args[0] === "--version") {
  console.log("ccusage-codex 18.0.11");
  process.exit(0);
}
if (args[0] === "daily" && args.includes("--json") && args.includes("--offline")) {
  process.stdout.write(fs.readFileSync(path.join(here, "..", "ccusage-codex-daily.json"), "utf8"));
  process.exit(0);
}
console.error(`unsupported ccusage-codex fixture args: ${args.join(" ")}`);
process.exit(2);
```

`tests/fixtures/helpers/ccusage-fails.mjs`:

```javascript
#!/usr/bin/env node
console.error("fixture helper failed");
process.exit(42);
```

`tests/fixtures/helpers/ccusage-prompts.mjs`:

```javascript
#!/usr/bin/env node
console.log(JSON.stringify({
  daily: [{
    date: "2026-05-09",
    inputTokens: 100,
    outputTokens: 200,
    cacheCreationTokens: 50,
    cacheReadTokens: 10000,
    totalCost: 0.12,
    modelsUsed: ["claude-sonnet-4"],
    prompt: "secret prompt",
    response: "secret response",
    toolCall: { arguments: "secret tool payload" }
  }]
}));
```

`tests/fixtures/helpers/ccusage-invalid-json.mjs`:

```javascript
#!/usr/bin/env node
process.stdout.write("{ invalid json with secret prompt text ");
```

`tests/fixtures/helpers/ccusage-secret-stderr.mjs`:

```javascript
#!/usr/bin/env node
console.error("helper failed while reading secret prompt text and secret response text");
process.exit(43);
```

- [ ] **Step 2: Add JSON fixtures**

`tests/fixtures/ccusage-daily.json`:

```json
{
  "daily": [
    {
      "date": "2026-05-08",
      "inputTokens": 1000,
      "outputTokens": 2000,
      "cacheCreationTokens": 300,
      "cacheReadTokens": 40000,
      "totalTokens": 43300,
      "totalCost": 1.25,
      "modelsUsed": ["claude-sonnet-4"]
    },
    {
      "date": "2026-05-09",
      "inputTokens": 1500,
      "outputTokens": 2500,
      "cacheCreationTokens": 500,
      "cacheReadTokens": 80000,
      "totalTokens": 84500,
      "totalCost": 2.50,
      "modelsUsed": ["claude-opus-4", "claude-sonnet-4"]
    }
  ]
}
```

`tests/fixtures/ccusage-daily-next.json`:

```json
{
  "daily": [
    {
      "date": "2026-05-08",
      "inputTokens": 1000,
      "outputTokens": 2000,
      "cacheCreationTokens": 300,
      "cacheReadTokens": 40000,
      "totalTokens": 43300,
      "totalCost": 1.25,
      "modelsUsed": ["claude-sonnet-4"]
    },
    {
      "date": "2026-05-09",
      "inputTokens": 1700,
      "outputTokens": 3100,
      "cacheCreationTokens": 700,
      "cacheReadTokens": 90000,
      "totalTokens": 95500,
      "totalCost": 2.95,
      "modelsUsed": ["claude-opus-4", "claude-sonnet-4"]
    }
  ]
}
```

`tests/fixtures/ccusage-codex-daily.json`:

```json
{
  "daily": [
    {
      "date": "2026-05-09",
      "inputTokens": 700,
      "outputTokens": 900,
      "cachedInputTokens": 5000,
      "reasoningOutputTokens": 125,
      "totalTokens": 6600,
      "costUSD": 0.40,
      "models": {
        "gpt-5.2-codex": {
          "inputTokens": 700,
          "outputTokens": 900,
          "cachedInputTokens": 5000,
          "reasoningOutputTokens": 125,
          "totalTokens": 6600,
          "isFallback": false
        }
      }
    }
  ]
}
```

- [ ] **Step 3: Write provider tests**

Add `tests/usage_provider.rs`:

```rust
use glorp::storage::usage_store::UsageStore;
use glorp::usage::ccusage::{CcusageCommandProvider, HelperDiscovery, HelperPaths};
use glorp::usage::provider::UsageProvider;
use tempfile::tempdir;

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/helpers").join(name)
}

fn provider(claude: Option<&str>, codex: Option<&str>) -> CcusageCommandProvider {
    CcusageCommandProvider::new(HelperPaths {
        claude: claude.map(fixture),
        codex: codex.map(fixture),
        node: None,
    })
}

#[test]
fn provider_normalizes_claude_and_codex_records() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider(Some("ccusage-ok.mjs"), Some("ccusage-codex-ok.mjs"));
    let result = provider.poll(&mut store).unwrap();
    assert!(result.deltas.iter().any(|d| d.provider_surface == "claude-code"));
    assert!(result.deltas.iter().any(|d| d.provider_surface == "codex"));
    assert!(result.deltas.iter().all(|d| d.confidence == "local-log-derived"));
    assert!(result.deltas.iter().any(|d| d.model.as_deref() == Some("gpt-5.2-codex")));
}

#[test]
fn repeated_poll_does_not_double_count_unchanged_totals() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider(Some("ccusage-ok.mjs"), None);
    let first = provider.poll(&mut store).unwrap();
    let second = provider.poll(&mut store).unwrap();
    assert!(first.total_effective_tokens > 0.0);
    assert_eq!(second.total_effective_tokens, 0.0);
}

#[test]
fn poll_with_increased_same_day_total_emits_only_increment() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let mut provider = provider(Some("ccusage-ok.mjs"), None);
    let first = provider.poll(&mut store).unwrap();
    assert!(first.total_effective_tokens > 0.0);

    provider.set_extra_env_for_test([("CCUSAGE_FIXTURE", "ccusage-daily-next.json")]);
    let second = provider.poll(&mut store).unwrap();

    // 2026-05-09 increased by input 200 + output 600 + cache creation 200
    // + cache reads 10000 * 0.03.
    assert_eq!(second.total_effective_tokens, 1300.0);
    assert_eq!(second.deltas.len(), 1);
}

#[test]
fn helper_failure_returns_diagnostic_without_delta() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider(Some("ccusage-fails.mjs"), None);
    let result = provider.poll(&mut store).unwrap();
    assert_eq!(result.total_effective_tokens, 0.0);
    assert!(result.diagnostics.iter().any(|d| d.code == "helper_exit"));
}

#[test]
fn transcript_like_fields_are_ignored() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let provider = provider(Some("ccusage-prompts.mjs"), None);
    let result = provider.poll(&mut store).unwrap();
    assert_eq!(result.diagnostics.len(), 0);
    let stored = store.recent_events(10).unwrap();
    let rendered = serde_json::to_string(&stored).unwrap();
    assert!(!rendered.contains("secret prompt"));
    assert!(!rendered.contains("secret response"));
    assert!(!rendered.contains("secret tool payload"));
}

#[test]
fn invalid_json_and_helper_stderr_are_sanitized() {
    let dir = tempdir().unwrap();
    let mut store = UsageStore::open(&dir.path().join("usage.sqlite")).unwrap();
    let invalid = provider(Some("ccusage-invalid-json.mjs"), None).poll(&mut store).unwrap();
    let stderr = provider(Some("ccusage-secret-stderr.mjs"), None).poll(&mut store).unwrap();
    let rendered = format!("{:?}{:?}", invalid.diagnostics, stderr.diagnostics);
    assert!(rendered.contains("invalid_json"));
    assert!(rendered.contains("helper_exit"));
    assert!(!rendered.contains("secret prompt"));
    assert!(!rendered.contains("secret response"));
}

#[test]
fn helper_discovery_prefers_env_then_path_without_reading_real_logs() {
    let env_path = fixture("ccusage-ok.mjs");
    let path_path = fixture("ccusage-fails.mjs");
    let discovered = HelperDiscovery::from_sources(
        [("GLORP_CCUSAGE_BIN", env_path.as_path())],
        [path_path.as_path()],
    )
    .unwrap();
    assert_eq!(discovered.claude.unwrap(), env_path);
}
```

- [ ] **Step 4: Run provider tests to confirm failure**

Run:

```bash
cargo test --test usage_provider
```

Expected: FAIL with unresolved `usage` module.

- [ ] **Step 5: Implement provider interfaces**

Add `src/usage/provider.rs`:

```rust
use crate::error::Result;
use crate::storage::usage_store::UsageStore;

#[derive(Debug, Clone)]
pub struct UsagePollResult {
    pub deltas: Vec<UsageDelta>,
    pub diagnostics: Vec<ProviderDiagnostic>,
    pub total_effective_tokens: f64,
}

#[derive(Debug, Clone)]
pub struct UsageDelta {
    pub provider_surface: String,
    pub effective_tokens: f64,
    pub confidence: String,
    pub period_start: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProviderDiagnostic {
    pub provider_surface: String,
    pub code: String,
    pub message: String,
}

pub trait UsageProvider {
    fn poll(&self, store: &mut UsageStore) -> Result<UsagePollResult>;
}
```

- [ ] **Step 6: Implement tolerant JSON normalization**

Add `src/usage/normalize.rs`. Normalize `daily` arrays with Claude-style fields `inputTokens`, `outputTokens`, `cacheCreationTokens`, `cacheReadTokens`, `totalCost`, `modelsUsed`, and `modelBreakdowns`; Codex-style fields `cachedInputTokens`, `reasoningOutputTokens`, `costUSD`, and `models`; and wrapper arrays named either `daily` or `data`. Ignore unknown fields and transcript-like fields. Treat `cachedInputTokens` as cache-read tokens. Default absent `cacheCreationTokens` to `0` for Codex rows. Preserve `reasoningOutputTokens` in normalized records for inspection, but do not add it to effective-token food/XP. If required token buckets are absent, return a structured `missing_token_fields` diagnostic whose message contains field names only, not raw JSON.

- [ ] **Step 7: Implement command provider and discovery**

Add `src/usage/ccusage.rs`. The provider must:

- Prefer explicit helper paths from `GLORP_CCUSAGE_BIN` and `GLORP_CCUSAGE_CODEX_BIN`.
- Accept helper paths passed by the npm launcher through the same environment variables.
- Fall back to PATH discovery for `ccusage` and `ccusage-codex`.
- Represent helpers as a command plus argument prefix, not just a path:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperCommand {
    pub program: std::path::PathBuf,
    pub args_prefix: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HelperPaths {
    pub claude: Option<std::path::PathBuf>,
    pub codex: Option<std::path::PathBuf>,
    pub node: Option<std::path::PathBuf>,
}
```

- If an env-provided or npm-provided helper path ends in `.js` or `.mjs`, execute it as `node <helper> ...` using `GLORP_NODE_BIN` when set, then PATH `node`.
- If helper discovery finds an executable command on PATH, execute that command directly.
- Add `set_extra_env_for_test` behind `#[cfg(test)]` or as a crate-visible test hook so fixture tests can switch from `ccusage-daily.json` to `ccusage-daily-next.json` without mutating process-global environment.
- Invoke Claude Code helper as `ccusage daily --json --offline --order asc`.
- Invoke Codex helper as `ccusage-codex daily --json --offline`.
- Run `--version` separately when available and store the parser version.
- Sort normalized rows by parsed period in Rust before cursor diffing.
- Key cursors by `provider_surface`, command name, parser version, period start/date, and model when available.
- Store the previous raw bucket totals per cursor key. On each poll, emit only positive deltas for each raw bucket. If a bucket total decreases, emit a sanitized `cursor_total_decreased` diagnostic and do not create negative food/XP.
- Return structured diagnostics for missing helper, non-zero exit, invalid JSON, missing token fields, and cursor corruption.
- Sanitize all diagnostics before persisting or printing: messages may include provider name, exit status, diagnostic code, and safe field names; they must never include raw stdout/stderr snippets, prompt text, response text, tool-call payloads, or JSON parse excerpts.

- [ ] **Step 8: Export usage module**

Modify `src/lib.rs`:

```rust
pub mod usage;
```

Add `src/usage/mod.rs`:

```rust
pub mod ccusage;
pub mod normalize;
pub mod provider;
```

- [ ] **Step 9: Run provider tests**

Run:

```bash
cargo test --test usage_provider
```

Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add src/usage tests/usage_provider.rs tests/fixtures src/lib.rs
git commit -m "feat: ingest usage through ccusage helpers"
```

## Task 4: Effective Token Model

**Stories:** `story-004`

**Files:**
- Create: `src/game/mod.rs`
- Create: `src/game/effective_tokens.rs`
- Create: `tests/game_rules.rs`
- Modify: `src/lib.rs`
- Modify: `src/usage/normalize.rs`

- [ ] **Step 1: Write effective-token tests**

Add the first tests to `tests/game_rules.rs`:

```rust
use glorp::config::AppConfig;
use glorp::game::effective_tokens::{EffectiveTokenWeights, TokenBuckets};

#[test]
fn effective_tokens_count_cache_reads_lightly() {
    let weights = EffectiveTokenWeights::default();
    let regular = TokenBuckets {
        uncached_input: 1000,
        output: 1000,
        cache_creation: 500,
        cache_read: 0,
        reasoning_output: 0,
    };
    let cache_heavy = TokenBuckets { cache_read: 2500, ..regular };
    assert_eq!(weights.compute(regular), 2500.0);
    assert_eq!(weights.cache_read_weight, 0.03);
    assert!(weights.compute(cache_heavy) < 5000.0);
}

#[test]
fn cache_read_weight_is_configurable() {
    let config = AppConfig { cache_read_weight: 0.05 };
    let weights = EffectiveTokenWeights::from_config(config);
    let buckets = TokenBuckets {
        uncached_input: 0,
        output: 0,
        cache_creation: 0,
        cache_read: 1000,
        reasoning_output: 999_999,
    };
    assert_eq!(weights.compute(buckets), 50.0);
}

#[test]
fn cost_never_changes_effective_tokens() {
    let weights = EffectiveTokenWeights::default();
    let buckets = TokenBuckets {
        uncached_input: 100,
        output: 200,
        cache_creation: 300,
        cache_read: 1000,
        reasoning_output: 50,
    };
    let with_low_cost = weights.compute_with_display_cost(buckets, Some(0.01));
    let with_high_cost = weights.compute_with_display_cost(buckets, Some(999.99));
    assert_eq!(with_low_cost.effective_tokens, with_high_cost.effective_tokens);
}

#[test]
fn missing_buckets_default_to_zero() {
    let buckets = TokenBuckets::default();
    assert_eq!(EffectiveTokenWeights::default().compute(buckets), 0.0);
}
```

- [ ] **Step 2: Run tests to confirm failure**

Run:

```bash
cargo test --test game_rules effective_tokens
```

Expected: FAIL with unresolved `game` module.

- [ ] **Step 3: Implement the model**

Add `src/game/mod.rs`:

```rust
pub mod effective_tokens;
```

Add `src/game/effective_tokens.rs`:

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TokenBuckets {
    pub uncached_input: u64,
    pub output: u64,
    pub cache_creation: u64,
    pub cache_read: u64,
    pub reasoning_output: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectiveTokenWeights {
    pub cache_read_weight: f64,
}

impl Default for EffectiveTokenWeights {
    fn default() -> Self {
        Self { cache_read_weight: 0.03 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectiveTokenResult {
    pub effective_tokens: f64,
    pub display_cost_usd: Option<f64>,
}

impl EffectiveTokenWeights {
    pub fn compute(&self, buckets: TokenBuckets) -> f64 {
        buckets.uncached_input as f64
            + buckets.output as f64
            + buckets.cache_creation as f64
            + self.cache_read_weight * buckets.cache_read as f64
    }

    pub fn compute_with_display_cost(
        &self,
        buckets: TokenBuckets,
        display_cost_usd: Option<f64>,
    ) -> EffectiveTokenResult {
        EffectiveTokenResult {
            effective_tokens: self.compute(buckets),
            display_cost_usd,
        }
    }
}
```

Reasoning output is preserved in `TokenBuckets` and normalized records for display/debugging when providers expose it, but it is not part of MVP effective-token food/XP unless a later product decision changes the approved formula.

Also add:

```rust
impl EffectiveTokenWeights {
    pub fn from_config(config: crate::config::AppConfig) -> Self {
        Self { cache_read_weight: config.cache_read_weight }
    }
}
```

- [ ] **Step 4: Use explicit buckets in normalization**

Modify `src/usage/normalize.rs` so normalized events carry `TokenBuckets` and compute `effective_tokens` only through `EffectiveTokenWeights::compute_with_display_cost`.

- [ ] **Step 5: Export game module and run tests**

Modify `src/lib.rs`:

```rust
pub mod game;
```

Run:

```bash
cargo test --test game_rules effective_tokens
cargo test --test usage_provider
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/game src/usage/normalize.rs src/lib.rs tests/game_rules.rs
git commit -m "feat: add effective token metabolism input"
```

## Task 5: Deterministic Pet Generation, Init, Rename, And Reset

**Stories:** `story-003`, part of `story-002`

**Files:**
- Create: `src/pet/mod.rs`
- Create: `src/pet/generation.rs`
- Create: `tests/generation.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/commands/init.rs`
- Modify: `src/commands/reset.rs`
- Modify: `src/lib.rs`
- Modify: `src/storage/state.rs`

- [ ] **Step 1: Write deterministic generation tests**

Add `tests/generation.rs`:

```rust
use glorp::pet::generation::{generate_pet, resolve_accepted_name, Species};

#[test]
fn same_seed_generates_same_pet() {
    let a = generate_pet("mochi-7f3a");
    let b = generate_pet("mochi-7f3a");
    assert_eq!(a, b);
}

#[test]
fn mvp_species_are_available() {
    let all = Species::all();
    assert!(all.contains(&Species::Fuzz));
    assert!(all.contains(&Species::Blob));
    assert!(all.contains(&Species::Ghost));
    assert!(all.contains(&Species::Glitch));
    assert!(all.contains(&Species::Crystal));
    assert!(all.contains(&Species::Mech));
    assert_eq!(all.len(), 6);
}

#[test]
fn species_names_have_distinct_grammar() {
    let fuzz = generate_pet("force-fuzz-1").with_species_for_test(Species::Fuzz);
    let mech = generate_pet("force-mech-1").with_species_for_test(Species::Mech);
    assert_ne!(fuzz.generated_name, mech.generated_name);
    assert!(fuzz.generated_name.chars().all(|c| c.is_ascii_lowercase()));
    assert!(mech.generated_name.chars().any(|c| c.is_ascii_digit()) || mech.generated_name.contains('-'));
}

#[test]
fn hatching_name_decision_accepts_generated_or_replacement_name() {
    let pet = generate_pet("mochi-7f3a");
    assert_eq!(resolve_accepted_name(&pet.generated_name, None), pet.generated_name);
    assert_eq!(resolve_accepted_name(&pet.generated_name, Some("sprig")), "sprig");
}
```

- [ ] **Step 2: Write CLI init/reset tests**

Extend `tests/cli_smoke.rs`:

```rust
#[test]
fn init_creates_state_and_blocks_second_init() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success()
        .stdout(predicate::str::contains("mochi has hatched"));

    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["init", "--seed", "other"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already has a pet"));
}

#[test]
fn reset_requires_confirmation_and_removes_pet_state() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .arg("reset")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--yes"));

    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["reset", "--yes"])
        .assert()
        .success();

    assert!(!dir.path().join("state.json").exists());
}

#[test]
fn init_with_confirmed_reinit_replaces_pet_state_without_touching_usage_db() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();
    std::fs::write(dir.path().join("usage.sqlite"), "sentinel usage db").unwrap();

    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["init", "--seed", "ori-shard", "--name", "ori", "--yes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ori has hatched"));

    let state = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(state.contains("ori-shard"));
    assert_eq!(std::fs::read_to_string(dir.path().join("usage.sqlite")).unwrap(), "sentinel usage db");
}

#[test]
fn rename_changes_display_name_without_changing_seed() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["rename", "sprig"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sprig"));

    let state = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(state.contains("mochi-7f3a"));
    assert!(state.contains("sprig"));
    assert!(!state.contains("\"accepted_name\":\"mochi\""));
}
```

- [ ] **Step 3: Run tests to confirm failure**

Run:

```bash
cargo test --test generation
cargo test --test cli_smoke init reset
```

Expected: FAIL with unresolved `pet` and command implementations.

- [ ] **Step 4: Implement generation**

Add `src/pet/mod.rs`:

```rust
pub mod generation;
```

Add `src/pet/generation.rs` with:

- `Species` enum for the six MVP species.
- Stable FNV-1a seed hash and deterministic RNG derived from it.
- Seed-selected `eyes`, `mouth`, `pattern`, `accent`, `palette_index`, `morph_index`, `animation_phase`.
- Species-aware name grammar:
  - fuzz/blob names use soft syllables such as `mo`, `puff`, `kib`, `lu`, `chi`, `lo`, `mi`.
  - ghost names use airy syllables such as `wisp`, `veil`, `noct`, `oma`.
  - glitch names use clipped digital syllables such as `bit`, `hex`, `vex`, `0x`.
  - crystal names use mineral syllables such as `ori`, `shard`, `lux`, `facet`.
  - mech names use mechanical syllables and digits such as `axl`, `bolt`, `rivet`, `07`.
- `resolve_accepted_name(generated_name, replacement)` returns the replacement when provided and otherwise accepts the generated name. The TTY prompt in `glorp init` must call this same logic so accept/rename behavior is testable without a fragile terminal harness.

- [ ] **Step 5: Implement command dispatch**

Create `src/commands/mod.rs`, `src/commands/init.rs`, `src/commands/reset.rs`, `src/commands/status.rs`, `src/commands/doctor.rs`, and `src/commands/watch.rs`.

Modify `src/lib.rs` so `run()` dispatches each CLI command into `commands::*`.

`glorp init` behavior:

- Resolve `AppPaths`, create config dir, open usage DB.
- If state exists and `--yes` is false, fail with an actionable error.
- If state exists and `--yes` is true, replace only pet state and preserve `usage.sqlite`.
- Generate a seed when none is supplied.
- Generate pet traits from seed.
- If `--name` is supplied, use it. If not and stdin is a TTY, print the generated name and ask for accept/rename. If not a TTY, use the generated name.
- Before saving the pet, read historical normalized usage from `usage.sqlite` when present and compute `CalibrationBaseline::from_history(...)` plus `RhythmProfile::from_history(...)`. If no usage history exists yet, use defaults.
- Save stage `s0`, XP `0.0`, default vitals `fed=70`, `happiness=70`, `energy=70`, generated species, calibration baseline, and rhythm profile.
- Do not grant XP, food, vitals, or evolution from historical usage. Historical usage only calibrates the baseline/rhythm.

`glorp rename <name>` behavior:

- Load existing pet state.
- Change `accepted_name` only.
- Preserve seed and generated traits.

`glorp reset --yes` behavior:

- Delete `state.json`.
- Keep `usage.sqlite` so ccusage can be conservatively re-read after a new init.
- Print that only Glorp pet state was reset.

- [ ] **Step 6: Run generation and CLI tests**

Run:

```bash
cargo test --test generation
cargo test --test cli_smoke
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/pet src/commands src/lib.rs src/storage/state.rs tests/generation.rs tests/cli_smoke.rs
git commit -m "feat: add deterministic pet init and reset"
```

## Task 6: Calibration And Seven-Stage Evolution

**Stories:** `story-005`

**Files:**
- Create: `src/game/calibration.rs`
- Create: `src/game/evolution.rs`
- Modify: `src/game/mod.rs`
- Modify: `src/commands/init.rs`
- Modify: `src/storage/state.rs`
- Modify: `tests/cli_smoke.rs`
- Modify: `tests/game_rules.rs`

- [ ] **Step 1: Add calibration and evolution tests**

Append to `tests/game_rules.rs`:

```rust
use glorp::game::calibration::{CalibrationBaseline, DailyUsage};
use glorp::game::evolution::{apply_xp_delta, stage_for_xp, Stage};
use time::macros::date;

#[test]
fn historical_usage_calibrates_but_does_not_grant_initial_xp() {
    let history = vec![
        DailyUsage::new(date!(2026-04-27), 100_000.0),
        DailyUsage::new(date!(2026-04-28), 120_000.0),
        DailyUsage::new(date!(2026-04-29), 500_000.0),
    ];
    let baseline = CalibrationBaseline::from_history(&history);
    assert!(baseline.daily_effective_tokens >= 100_000.0);
    assert_eq!(stage_for_xp(0.0), Stage::S0);
}

#[test]
fn low_and_high_usage_users_progress_by_relative_effort() {
    let low = CalibrationBaseline { daily_effective_tokens: 50_000.0 };
    let high = CalibrationBaseline { daily_effective_tokens: 500_000_000.0 };
    let low_xp = (0..35).fold(0.0, |xp, _| apply_xp_delta(xp, 50_000.0, low).xp);
    let high_xp = (0..35).fold(0.0, |xp, _| apply_xp_delta(xp, 500_000_000.0, high).xp);
    assert_eq!(stage_for_xp(low_xp), stage_for_xp(high_xp));
}

#[test]
fn extreme_bucket_cannot_skip_most_of_lifecycle() {
    let baseline = CalibrationBaseline { daily_effective_tokens: 100_000.0 };
    let result = apply_xp_delta(0.0, 100_000_000.0, baseline);
    assert!(result.stage_transitions.len() <= 2);
    assert!(result.mood_food_benefit <= 25.0);
}

#[test]
fn stage_transition_event_is_recorded_once() {
    let baseline = CalibrationBaseline { daily_effective_tokens: 100_000.0 };
    let before = apply_xp_delta(0.0, 100_000.0, baseline);
    let after = apply_xp_delta(before.xp, 100_000.0, baseline);
    let total_events = before.stage_transitions.len() + after.stage_transitions.len();
    assert_eq!(total_events, 1);
}
```

Also add a command-level test to `tests/cli_smoke.rs` after calibration exists:

```rust
#[test]
fn init_uses_historical_usage_for_calibration_without_initial_xp() {
    let dir = tempfile::tempdir().unwrap();
    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", "tests/fixtures/helpers/ccusage-ok.mjs")
        .env("GLORP_CCUSAGE_CODEX_BIN", "tests/fixtures/helpers/ccusage-codex-ok.mjs")
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    let state = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(state.contains("\"stage\":\"s0\""));
    assert!(state.contains("\"xp\":0.0"));
    assert!(state.contains("\"calibration\""));
    assert!(state.contains("\"daily_effective_tokens\""));
}
```

- [ ] **Step 2: Run tests to confirm failure**

Run:

```bash
cargo test --test game_rules calibration evolution
```

Expected: FAIL with unresolved modules.

- [ ] **Step 3: Implement calibration**

Add `src/game/calibration.rs`:

- Ignore zero-use days for active-day median.
- Use the median of recent active days when at least five active days exist.
- Fall back to `100_000.0` effective tokens per active day.
- Persist baseline in `state.json` so watch/status use the same curve until refreshed.

Core shape:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalibrationBaseline {
    pub daily_effective_tokens: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DailyUsage {
    pub day: time::Date,
    pub effective_tokens: f64,
}
```

Derive `serde::Serialize` and `serde::Deserialize` for `CalibrationBaseline`, `DailyUsage`, and any persisted rhythm summary so they can be embedded in `PetState`.

- [ ] **Step 4: Implement evolution**

Add `src/game/evolution.rs`:

- Use seven stages `S0` through `S6`.
- Use thresholds in calibrated active-day XP units: `0.0`, `0.25`, `1.0`, `3.0`, `7.0`, `21.0`, `49.0`.
- Convert effective-token buckets to calibrated XP units with diminishing returns:

```rust
pub fn calibrated_xp_units(delta_effective: f64, baseline: CalibrationBaseline) -> f64 {
    let daily = baseline.daily_effective_tokens.max(1.0);
    let relative = (delta_effective / daily).max(0.0);
    let direct = relative.min(0.25);
    let excess = (relative - 0.25).max(0.0);
    direct + excess.sqrt() * 0.05
}
```

- Cap food/mood benefit from a single bucket separately from XP.
- Produce `StageTransition { from, to }` events only for newly crossed thresholds.

- [ ] **Step 5: Export game modules and wire state**

Modify `src/game/mod.rs`:

```rust
pub mod calibration;
pub mod effective_tokens;
pub mod evolution;
```

Modify `PetState` to include `calibration: CalibrationBaseline`, `rhythm: RhythmProfile`, and `seen_stage_transitions: Vec<String>`.

Modify `src/commands/init.rs` so `glorp init` performs a provider history read before saving the pet:

- Use `CcusageCommandProvider` to collect historical normalized records when helpers are available.
- Store those records/cursors as historical calibration data.
- Compute and persist `CalibrationBaseline::from_history(...)` and `RhythmProfile::from_history(...)`.
- Leave stage as `s0`, XP as `0.0`, vitals at defaults, and no evolution events recorded.
- If helpers are unavailable or fail, init still creates a pet with default calibration and stores a diagnostic; watch/status will show the blocked setup state.

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test --test game_rules
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/game src/storage/state.rs tests/game_rules.rs
git commit -m "feat: calibrate evolution to user usage"
```

## Task 7: Mood, Decay, Vitals, And Wilted Recovery

**Stories:** `story-006`

**Files:**
- Create: `src/game/metabolism.rs`
- Modify: `src/game/mod.rs`
- Modify: `tests/game_rules.rs`
- Modify: `src/commands/watch.rs`

- [ ] **Step 1: Add metabolism tests**

Append to `tests/game_rules.rs`:

```rust
use glorp::game::metabolism::{apply_decay, apply_food, Mood, RhythmProfile, Vitals};
use time::macros::datetime;

#[test]
fn effective_usage_improves_vitals_without_exceeding_caps() {
    let vitals = Vitals { fed: 35.0, happiness: 40.0, energy: 30.0 };
    let out = apply_food(vitals, 25_000.0, 100_000.0);
    assert!(out.vitals.fed > vitals.fed);
    assert!(out.vitals.happiness > vitals.happiness);
    assert!(out.vitals.energy > vitals.energy);
    assert!(out.vitals.fed <= 100.0);
    assert!(out.vitals.happiness <= 100.0);
    assert!(out.vitals.energy <= 100.0);
}

#[test]
fn same_day_gap_does_not_jump_to_wilted() {
    let vitals = Vitals { fed: 70.0, happiness: 70.0, energy: 70.0 };
    let out = apply_decay(
        vitals,
        datetime!(2026-05-09 09:00 UTC),
        datetime!(2026-05-09 13:00 UTC),
        RhythmProfile::default(),
    );
    assert_ne!(out.mood, Mood::Wilted);
    assert!(out.vitals.fed > 35.0);
}

#[test]
fn overnight_and_weekend_decay_is_slower() {
    let vitals = Vitals { fed: 70.0, happiness: 70.0, energy: 70.0 };
    let overnight = apply_decay(vitals, datetime!(2026-05-08 22:00 UTC), datetime!(2026-05-09 06:00 UTC), RhythmProfile::default());
    let workday = apply_decay(vitals, datetime!(2026-05-08 09:00 UTC), datetime!(2026-05-08 17:00 UTC), RhythmProfile::default());
    assert!(overnight.vitals.fed > workday.vitals.fed);
}

#[test]
fn weekend_heavy_profile_learns_weekend_activity() {
    let profile = RhythmProfile::from_active_hours(&[
        datetime!(2026-05-03 11:00 UTC),
        datetime!(2026-05-04 12:00 UTC),
        datetime!(2026-05-10 13:00 UTC),
        datetime!(2026-05-11 14:00 UTC),
    ]);
    assert!(profile.weekend_activity_weight > RhythmProfile::default().weekend_activity_weight);
}

#[test]
fn historically_inactive_hours_decay_slowly() {
    let profile = RhythmProfile::from_active_hours(&[
        datetime!(2026-05-05 09:00 UTC),
        datetime!(2026-05-06 10:00 UTC),
        datetime!(2026-05-07 11:00 UTC),
    ]);
    let vitals = Vitals { fed: 70.0, happiness: 70.0, energy: 70.0 };
    let inactive_window = apply_decay(vitals, datetime!(2026-05-08 02:00 UTC), datetime!(2026-05-08 05:00 UTC), profile);
    let active_window = apply_decay(vitals, datetime!(2026-05-08 09:00 UTC), datetime!(2026-05-08 12:00 UTC), profile);
    assert!(inactive_window.vitals.fed > active_window.vitals.fed);
}

#[test]
fn sustained_absence_can_wilt_but_real_usage_recovers() {
    let vitals = Vitals { fed: 20.0, happiness: 25.0, energy: 20.0 };
    let wilted = apply_decay(
        vitals,
        datetime!(2026-05-01 09:00 UTC),
        datetime!(2026-05-09 09:00 UTC),
        RhythmProfile::default(),
    );
    assert_eq!(wilted.mood, Mood::Wilted);
    let recovered = apply_food(wilted.vitals, 50_000.0, 100_000.0);
    assert_ne!(recovered.mood, Mood::Wilted);
}

#[test]
fn there_is_no_death_transition() {
    let vitals = Vitals { fed: 0.0, happiness: 0.0, energy: 0.0 };
    let out = apply_decay(vitals, datetime!(2026-01-01 00:00 UTC), datetime!(2026-05-09 00:00 UTC), RhythmProfile::default());
    assert_eq!(out.mood, Mood::Wilted);
    assert!(!format!("{:?}", out).to_lowercase().contains("death"));
}
```

- [ ] **Step 2: Run tests to confirm failure**

Run:

```bash
cargo test --test game_rules metabolism
```

Expected: FAIL with unresolved `metabolism`.

- [ ] **Step 3: Implement metabolism**

Add `src/game/metabolism.rs`:

- `Mood` enum: `Happy`, `Content`, `Hungry`, `Sad`, `Sleepy`, `Wilted`.
- No `Dead`, `Graveyard`, `Revive`, or `Permadeath` types.
- Active-hour decay weight `1.0`.
- Overnight decay weight `0.35`.
- Weekend decay weight `0.45` unless calibration shows regular weekend usage, in which case use `0.75`.
- Historically inactive hour weight `0.20`.
- `RhythmProfile::from_history(...)` and `RhythmProfile::from_active_hours(...)` must learn:
  - active hour-of-day histogram from recent usage events,
  - whether weekends are normal active windows for this user,
  - historically inactive windows that should decay slowly,
  - conservative defaults until at least five active timestamps exist.
- Mood thresholds:
  - `fed < 25`: hungry.
  - `happiness < 35`: sad.
  - `energy < 20`: sleepy.
  - `fed < 12 && happiness < 20`: wilted.
  - sustained rhythm-relative absence with low vitals: wilted.

- [ ] **Step 4: Wire watch/status state updates through metabolism**

When usage deltas arrive, call `apply_food` and update the persisted rhythm profile with the active timestamp. On every watch render tick and on `status`, reconcile elapsed time through `apply_decay` and save updated vitals.

The optional TUI `p` key may create a short affection event by increasing happiness up to `+4`; it must not change fed, XP, or effective-token counters.

- [ ] **Step 5: Run metabolism tests**

Run:

```bash
cargo test --test game_rules
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/game/metabolism.rs src/game/mod.rs src/commands/watch.rs tests/game_rules.rs
git commit -m "feat: add gentle decay and wilted recovery"
```

## Task 8: Deterministic Pet Renderer And Animation

**Stories:** `story-008`

**Files:**
- Create: `src/pet/art.rs`
- Create: `src/pet/render.rs`
- Modify: `src/pet/mod.rs`
- Modify: `tests/generation.rs`

- [ ] **Step 1: Add renderer snapshot tests**

Append to `tests/generation.rs`:

```rust
use glorp::game::metabolism::Mood;
use glorp::game::evolution::Stage;
use glorp::pet::art::{morph_count, stage_label};
use glorp::pet::render::{AnimationFrame, closed_blink_eyes, palette_roles, render_pet, species_animation_profile};

#[test]
fn render_is_stable_for_same_seed_state_and_tick() {
    let pet = generate_pet("mochi-7f3a");
    let a = render_pet(&pet, Stage::S3, Mood::Content, AnimationFrame { tick: 42, compact: false });
    let b = render_pet(&pet, Stage::S3, Mood::Content, AnimationFrame { tick: 42, compact: false });
    assert_eq!(a, b);
}

#[test]
fn different_same_species_seeds_have_visible_variation() {
    let a = generate_pet("blob-a").with_species_for_test(Species::Blob);
    let b = generate_pet("blob-b").with_species_for_test(Species::Blob);
    let art_a = render_pet(&a, Stage::S5, Mood::Content, AnimationFrame { tick: 0, compact: false });
    let art_b = render_pet(&b, Stage::S5, Mood::Content, AnimationFrame { tick: 0, compact: false });
    assert_ne!(art_a.lines, art_b.lines);
}

#[test]
fn stage_labels_are_species_specific_across_shared_thresholds() {
    assert_ne!(stage_label(Species::Fuzz, Stage::S3), stage_label(Species::Mech, Stage::S3));
    assert_ne!(stage_label(Species::Ghost, Stage::S6), stage_label(Species::Crystal, Stage::S6));
}

#[test]
fn species_have_enough_seeded_morph_variety() {
    for species in Species::all() {
        assert!(morph_count(species, Stage::S1) >= 2);
        assert!(morph_count(species, Stage::S4) >= 3);
        assert!(morph_count(species, Stage::S6) >= 3);
    }
}

#[test]
fn compact_render_has_bounded_width() {
    let pet = generate_pet("mech-compact");
    let art = render_pet(&pet, Stage::S6, Mood::Wilted, AnimationFrame { tick: 9, compact: true });
    assert!(art.lines.iter().all(|line| line.chars().count() <= 18));
}

#[test]
fn evolution_event_has_renderable_celebration() {
    let pet = generate_pet("ori-shard");
    let art = render_pet(&pet, Stage::S4, Mood::Happy, AnimationFrame { tick: 1, compact: false }).with_evolution_flash(Stage::S3, Stage::S4);
    assert!(art.event_lines.iter().any(|line| line.contains("evolved")));
}

#[test]
fn palette_roles_follow_tokenpet_hue_offsets() {
    let pet = generate_pet("ori-shard");
    let roles = palette_roles(&pet);
    assert_eq!(roles.body.lightness, 0.84);
    assert_eq!(roles.body.base_chroma, 0.10);
    assert_eq!(roles.eye.hue_offset_degrees, 180);
    assert_eq!(roles.eye.lightness, 0.84);
    assert_eq!(roles.eye.base_chroma, 0.13);
    assert_eq!(roles.mouth.hue_offset_degrees, 30);
    assert_eq!(roles.accent.hue_offset_degrees, 90);
    assert_eq!(roles.pattern.hue_offset_degrees, 150);
}

#[test]
fn species_animation_profiles_match_tokenpet_mockup() {
    assert_eq!(species_animation_profile(Species::Fuzz).breath_period, 16);
    assert_eq!(species_animation_profile(Species::Fuzz).breath_hold, 4);
    assert_eq!(species_animation_profile(Species::Fuzz).blink_average, 32);
    assert_eq!(species_animation_profile(Species::Fuzz).blink_jitter, 12);
    assert_eq!(species_animation_profile(Species::Blob).breath_period, 13);
    assert_eq!(species_animation_profile(Species::Ghost).blink_average, 50);
    assert_eq!(species_animation_profile(Species::Glitch).breath_period, 9);
    assert_eq!(species_animation_profile(Species::Crystal).blink_jitter, 22);
    assert_eq!(species_animation_profile(Species::Mech).blink_average, 22);
}

#[test]
fn blink_is_seeded_desynchronized_and_mood_safe() {
    let pet = generate_pet("blink-seed").with_species_for_test(Species::Ghost);
    let a = render_pet(&pet, Stage::S3, Mood::Content, AnimationFrame { tick: 50, compact: false });
    let b = render_pet(&pet, Stage::S3, Mood::Content, AnimationFrame { tick: 51, compact: false });
    assert_ne!(a.lines, b.lines);
    assert_eq!(closed_blink_eyes(Species::Ghost), "— —");

    let sad = render_pet(&pet, Stage::S3, Mood::Sad, AnimationFrame { tick: 50, compact: false });
    let wilted = render_pet(&pet, Stage::S3, Mood::Wilted, AnimationFrame { tick: 50, compact: false });
    assert!(!sad.lines.join("\n").contains("— —"));
    assert!(!wilted.lines.join("\n").contains("— —"));
}
```

- [ ] **Step 2: Run tests to confirm failure**

Run:

```bash
cargo test --test generation render
```

Expected: FAIL with unresolved renderer.

- [ ] **Step 3: Implement art templates**

Add `src/pet/art.rs`:

- Define species-specific templates for stages `S0` through `S6`.
- Define species-specific stage labels through `stage_label(species, stage)`, while keeping the XP thresholds shared in `src/game/evolution.rs`.
- Give every species distinct `S0` and `S1` silhouette hints, so mech/crystal/ghost/fuzz/blob/glitch do not hatch from a shared generic egg.
- Provide at least two juvenile morph templates per species and at least three adult/sage morph templates per species. Seeded `morph_index` selects among these templates so same-species pets do not differ only by eyes or mouth.
- Use slots for `eyes`, `mouth`, `pattern`, and `accent`.
- Keep art data separate from game rules.
- Carry over mockup ideas:
  - fuzz: ears, whiskers, paws.
  - blob: round body and drips.
  - ghost: floating cloak and wisps.
  - glitch: blocky/noisy silhouette.
  - crystal: angular facets.
  - mech: antenna, chassis, treads.

- [ ] **Step 4: Implement renderer**

Add `src/pet/render.rs`:

- Return a `RenderedPet { lines: Vec<String>, spans: Vec<StyledSegment>, event_lines: Vec<String> }`.
- Apply mood eyes/mouth including wilted `,_,` and `_`.
- Apply seeded palette roles for body, eyes, mouth, accent, and pattern using the Tokenpet hue-offset model from `docs/tokenpet/project/pet.jsx`:
  - body: lightness `0.84`, base chroma `0.10 * saturation`, hue `seed_hue`.
  - eye: lightness `0.84`, base chroma `0.13 * saturation`, hue `seed_hue + 180`.
  - mouth: lightness `0.84`, base chroma `0.10 * saturation`, hue `seed_hue + 30`.
  - accent: lightness `0.82`, base chroma `0.11 * saturation`, hue `seed_hue + 90`.
  - pattern: lightness `0.76`, base chroma `0.06 * saturation`, hue `seed_hue + 150`.
- Use deterministic animation phases for breathing and blinking; blink must not occur on the same tick cadence as breathing.
- Implement the Tokenpet species animation profile constants:
  - fuzz: `breath_period=16`, `breath_hold=4`, `blink_average=32`, `blink_jitter=12`.
  - blob: `breath_period=13`, `breath_hold=5`, `blink_average=40`, `blink_jitter=14`.
  - ghost: `breath_period=11`, `breath_hold=3`, `blink_average=50`, `blink_jitter=18`.
  - glitch: `breath_period=9`, `breath_hold=2`, `blink_average=24`, `blink_jitter=8`.
  - crystal: `breath_period=19`, `breath_hold=6`, `blink_average=60`, `blink_jitter=22`.
  - mech: `breath_period=17`, `breath_hold=4`, `blink_average=22`, `blink_jitter=6`.
- Use species-specific closed-eye glyphs: fuzz/blob `- -`, ghost `— —`, glitch `▒▒▒`, crystal `◇ ◇`, mech `= =`.
- Suppress blink frames for `Sad`, `Sleepy`, and `Wilted` game moods, and for four ticks immediately after any mood change, so the pet never flashes a blink while changing expressions.
- Add species flavor in terminal-safe ways:
  - fuzz tail swish line.
  - blob drip.
  - ghost wisp.
  - glitch rare character corruption.
  - crystal sparkle.
  - mech LED/steam.
- In compact mode, crop or choose compact templates deliberately so text does not overlap.

- [ ] **Step 5: Export modules and run tests**

Modify `src/pet/mod.rs`:

```rust
pub mod art;
pub mod generation;
pub mod render;
```

Run:

```bash
cargo test --test generation
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/pet tests/generation.rs
git commit -m "feat: render deterministic animated pets"
```

## Task 9: Watch Mode Ratatui Shell And Tokenpet Visual Style

**Stories:** `story-007`, integrates `story-001` through `story-008`

**Files:**
- Create: `src/tui/mod.rs`
- Create: `src/tui/app.rs`
- Create: `src/tui/layout.rs`
- Create: `src/tui/style.rs`
- Create: `src/tui/view_model.rs`
- Create: `tests/style_tokens.rs`
- Create: `tests/tui_render.rs`
- Modify: `src/commands/watch.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write Tokenpet style-token tests**

Add `tests/style_tokens.rs`:

```rust
use glorp::tui::style::{semantic_styles, tokenpet_palette, LogKind};
use ratatui::style::Color;

#[test]
fn palette_matches_tokenpet_handoff_values() {
    let p = tokenpet_palette();
    assert_eq!(p.bg.source_oklch, "oklch(0.18 0.005 60)");
    assert_eq!(p.bg.rgb, Color::Rgb(0x13, 0x11, 0x0f));
    assert_eq!(p.surface.source_oklch, "oklch(0.22 0.006 60)");
    assert_eq!(p.surface.rgb, Color::Rgb(0x1d, 0x1a, 0x18));
    assert_eq!(p.fg.source_oklch, "oklch(0.94 0.01 80)");
    assert_eq!(p.fg.rgb, Color::Rgb(0xef, 0xeb, 0xe4));
    assert_eq!(p.dim.source_oklch, "oklch(0.66 0.012 70)");
    assert_eq!(p.dim.rgb, Color::Rgb(0x97, 0x91, 0x8a));
    assert_eq!(p.faint.source_oklch, "oklch(0.42 0.008 60)");
    assert_eq!(p.faint.rgb, Color::Rgb(0x50, 0x4c, 0x49));
    assert_eq!(p.accent.source_oklch, "oklch(0.78 0.14 70)");
    assert_eq!(p.accent.rgb, Color::Rgb(0xf0, 0xa6, 0x46));
    assert_eq!(p.good.source_oklch, "oklch(0.74 0.10 145)");
    assert_eq!(p.good.rgb, Color::Rgb(0x82, 0xbc, 0x83));
    assert_eq!(p.bad.source_oklch, "oklch(0.68 0.16 25)");
    assert_eq!(p.bad.rgb, Color::Rgb(0xea, 0x6a, 0x64));
}

#[test]
fn semantic_styles_preserve_tokenpet_roles() {
    let styles = semantic_styles();
    let p = tokenpet_palette();
    assert_eq!(styles.chrome_title.fg, Some(p.dim.rgb));
    assert_eq!(styles.chrome_title.bg, Some(p.surface.rgb));
    assert_eq!(styles.prompt_user.fg, Some(p.good.rgb));
    assert_eq!(styles.prompt_path.fg, Some(p.accent.rgb));
    assert_eq!(styles.section_header.fg, Some(p.faint.rgb));
    assert_eq!(styles.timestamp.fg, Some(p.faint.rgb));
    assert_eq!(styles.empty_bar.fg, Some(p.faint.rgb));
    assert_eq!(styles.filled_bar_good.fg, Some(p.good.rgb));
    assert_eq!(styles.filled_bar_accent.fg, Some(p.accent.rgb));
    assert_eq!(styles.event_rail_usage.fg, Some(p.good.rgb));
    assert_eq!(styles.event_rail_diagnostic.fg, Some(p.bad.rgb));
    assert_eq!(styles.sparkline_today.fg, Some(p.accent.rgb));
    assert_eq!(styles.sparkline_past.fg, Some(p.faint.rgb));
    assert_eq!(styles.log(LogKind::Normal).fg, Some(p.dim.rgb));
    assert_eq!(styles.log(LogKind::Usage).fg, Some(p.good.rgb));
    assert_eq!(styles.log(LogKind::Diagnostic).fg, Some(p.bad.rgb));
    assert_eq!(styles.log(LogKind::Evolution).fg, Some(p.accent.rgb));
}
```

- [ ] **Step 2: Run style tests to confirm failure**

Run:

```bash
cargo test --test style_tokens
```

Expected: FAIL with unresolved `tui::style`.

- [ ] **Step 3: Implement the Tokenpet style module**

Add `src/tui/style.rs` with:

```rust
use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenpetColor {
    pub name: &'static str,
    pub source_oklch: &'static str,
    pub rgb: Color,
}

#[derive(Debug, Clone, Copy)]
pub struct TokenpetPalette {
    pub bg: TokenpetColor,
    pub surface: TokenpetColor,
    pub fg: TokenpetColor,
    pub dim: TokenpetColor,
    pub faint: TokenpetColor,
    pub accent: TokenpetColor,
    pub good: TokenpetColor,
    pub bad: TokenpetColor,
}

pub fn tokenpet_palette() -> TokenpetPalette {
    TokenpetPalette {
        bg: TokenpetColor { name: "bg", source_oklch: "oklch(0.18 0.005 60)", rgb: Color::Rgb(0x13, 0x11, 0x0f) },
        surface: TokenpetColor { name: "surface", source_oklch: "oklch(0.22 0.006 60)", rgb: Color::Rgb(0x1d, 0x1a, 0x18) },
        fg: TokenpetColor { name: "fg", source_oklch: "oklch(0.94 0.01 80)", rgb: Color::Rgb(0xef, 0xeb, 0xe4) },
        dim: TokenpetColor { name: "dim", source_oklch: "oklch(0.66 0.012 70)", rgb: Color::Rgb(0x97, 0x91, 0x8a) },
        faint: TokenpetColor { name: "faint", source_oklch: "oklch(0.42 0.008 60)", rgb: Color::Rgb(0x50, 0x4c, 0x49) },
        accent: TokenpetColor { name: "accent", source_oklch: "oklch(0.78 0.14 70)", rgb: Color::Rgb(0xf0, 0xa6, 0x46) },
        good: TokenpetColor { name: "good", source_oklch: "oklch(0.74 0.10 145)", rgb: Color::Rgb(0x82, 0xbc, 0x83) },
        bad: TokenpetColor { name: "bad", source_oklch: "oklch(0.68 0.16 25)", rgb: Color::Rgb(0xea, 0x6a, 0x64) },
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LogKind {
    Normal,
    Usage,
    Diagnostic,
    Evolution,
    Help,
}

#[derive(Debug, Clone)]
pub struct SemanticStyles {
    pub chrome_title: Style,
    pub prompt_user: Style,
    pub prompt_path: Style,
    pub prompt_sep: Style,
    pub section_header: Style,
    pub timestamp: Style,
    pub primary_text: Style,
    pub label: Style,
    pub empty_bar: Style,
    pub filled_bar_good: Style,
    pub filled_bar_accent: Style,
    pub event_rail_usage: Style,
    pub event_rail_diagnostic: Style,
    pub sparkline_today: Style,
    pub sparkline_past: Style,
    pub overlay_border: Style,
}

impl SemanticStyles {
    pub fn log(&self, kind: LogKind) -> Style {
        let p = tokenpet_palette();
        match kind {
            LogKind::Normal => Style::default().fg(p.dim.rgb),
            LogKind::Usage => Style::default().fg(p.good.rgb),
            LogKind::Diagnostic => Style::default().fg(p.bad.rgb),
            LogKind::Evolution => Style::default().fg(p.accent.rgb).add_modifier(Modifier::BOLD),
            LogKind::Help => Style::default().fg(p.dim.rgb),
        }
    }
}

pub fn semantic_styles() -> SemanticStyles {
    let p = tokenpet_palette();
    SemanticStyles {
        chrome_title: Style::default().fg(p.dim.rgb).bg(p.surface.rgb),
        prompt_user: Style::default().fg(p.good.rgb),
        prompt_path: Style::default().fg(p.accent.rgb),
        prompt_sep: Style::default().fg(p.faint.rgb),
        section_header: Style::default().fg(p.faint.rgb),
        timestamp: Style::default().fg(p.faint.rgb),
        primary_text: Style::default().fg(p.fg.rgb),
        label: Style::default().fg(p.dim.rgb),
        empty_bar: Style::default().fg(p.faint.rgb),
        filled_bar_good: Style::default().fg(p.good.rgb),
        filled_bar_accent: Style::default().fg(p.accent.rgb),
        event_rail_usage: Style::default().fg(p.good.rgb),
        event_rail_diagnostic: Style::default().fg(p.bad.rgb),
        sparkline_today: Style::default().fg(p.accent.rgb).add_modifier(Modifier::BOLD),
        sparkline_past: Style::default().fg(p.faint.rgb),
        overlay_border: Style::default().fg(p.accent.rgb).bg(p.bg.rgb),
    }
}
```

The RGB approximations above are derived from the mockup's OKLCH values; keep the OKLCH strings as source-of-truth metadata so future style adjustments can trace back to `tokenpet.html`.

- [ ] **Step 4: Write TUI render tests**

Add `tests/tui_render.rs`:

```rust
use glorp::tui::app::{
    render_evolution_overlay_for_test, render_frame_for_test, render_hatch_overlay_for_test,
    render_help_overlay_for_test, run_single_watch_tick_for_test, WatchTestHarness, WatchViewModel,
};
use glorp::tui::style::tokenpet_palette;
use ratatui::{backend::TestBackend, buffer::Buffer, layout::Position, style::Color, Frame, Terminal};

fn buffer_lines(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| {
            let mut row = String::new();
            for x in 0..buf.area.width {
                if let Some(cell) = buf.cell(Position::new(buf.area.x + x, buf.area.y + y)) {
                    row.push_str(cell.symbol());
                }
            }
            row.trim_end().to_string()
        })
        .collect()
}

fn buffer_text(buf: &Buffer) -> String {
    buffer_lines(buf).join("\n")
}

fn has_cell(buf: &Buffer, symbol: &str, fg: Color) -> bool {
    (0..buf.area.height).any(|y| {
        (0..buf.area.width).any(|x| {
            buf.cell(Position::new(buf.area.x + x, buf.area.y + y))
                .map(|cell| cell.symbol() == symbol && cell.style().fg == Some(fg))
                .unwrap_or(false)
        })
    })
}

#[test]
fn wide_layout_has_tokenpet_chrome_panels_and_bars() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &WatchViewModel::fixture())).unwrap();
    let buf = terminal.backend().buffer();
    let p = tokenpet_palette();
    let text = buffer_text(buf);
    assert!(text.contains("glorp --"));
    assert!(text.contains("─ vitals"));
    assert!(text.contains("today"));
    assert!(text.contains("helper"));
    assert!(text.contains("●"));
    assert!(text.contains("█"));
    assert!(text.contains("░"));
    assert!(has_cell(buf, "█", p.good.rgb) || has_cell(buf, "█", p.accent.rgb));
    assert!(has_cell(buf, "░", p.faint.rgb));
}

#[test]
fn event_log_uses_timestamps_rails_sparkline_and_semantic_colors() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &WatchViewModel::fixture_with_events())).unwrap();
    let buf = terminal.backend().buffer();
    let p = tokenpet_palette();
    let text = buffer_text(buf);
    assert!(text.contains("13:42"));
    assert!(text.contains("▏"));
    assert!(text.contains("▁") || text.contains("▂") || text.contains("▃") || text.contains("▄") || text.contains("▅") || text.contains("▆") || text.contains("▇") || text.contains("█"));
    assert!(has_cell(buf, "▏", p.good.rgb));
    assert!(has_cell(buf, "▏", p.accent.rgb));
    assert!(has_cell(buf, "▏", p.bad.rgb));
    assert!(has_cell(buf, ":", p.faint.rgb));
}

#[test]
fn compact_boundary_is_exact_at_72_columns() {
    let mut at_72 = Terminal::new(TestBackend::new(72, 24)).unwrap();
    at_72.draw(|f| render_frame_for_test(f, &WatchViewModel::fixture())).unwrap();
    let lines_72 = buffer_lines(at_72.backend().buffer());
    assert!(lines_72.iter().any(|line| line.contains("vitals") && line.contains("today")));

    let mut at_71 = Terminal::new(TestBackend::new(71, 24)).unwrap();
    at_71.draw(|f| render_frame_for_test(f, &WatchViewModel::fixture())).unwrap();
    let lines_71 = buffer_lines(at_71.backend().buffer());
    let vitals_line = lines_71.iter().position(|line| line.contains("vitals")).unwrap();
    let today_line = lines_71.iter().position(|line| line.contains("today")).unwrap();
    assert!(today_line > vitals_line);
}

#[test]
fn small_height_degrades_without_text_overlap() {
    let backend = TestBackend::new(48, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &WatchViewModel::fixture())).unwrap();
    let lines = buffer_lines(terminal.backend().buffer());
    assert!(lines.iter().all(|line| line.chars().count() <= 48));
    let text = lines.join("\n");
    assert!(text.contains("glorp"));
    assert!(text.contains("q"));
    assert!(text.contains("?"));
}

#[test]
fn blocked_provider_state_renders_calm_setup_view() {
    let mut vm = WatchViewModel::fixture();
    vm.helper_status = "missing ccusage helper".into();
    vm.errors.push("install ccusage or use npm package with bundled helpers".into());
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_frame_for_test(f, &vm)).unwrap();
    let text = buffer_text(terminal.backend().buffer());
    assert!(text.contains("blocked"));
    assert!(text.contains("missing ccusage helper"));
}

#[test]
fn polling_tick_updates_activity_bucket_and_event_log() {
    let mut harness = WatchTestHarness::with_usage_delta("claude-code", "2026-05-09T13:42:00Z", 1300.0);
    let vm = run_single_watch_tick_for_test(&mut harness).unwrap();
    assert_eq!(vm.current_bucket_effective_tokens, 1300.0);
    assert!(vm.source_breakdown.iter().any(|source| source.name == "claude-code"));
    assert!(vm.recent_events.iter().any(|event| event.text.contains("1.3k effective tokens")));
}

#[test]
fn help_evolution_and_hatch_overlays_use_tokenpet_surface_and_accent() {
    let p = tokenpet_palette();
    fn assert_overlay(render: fn(&mut Frame<'_>), accent: Color) {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f)).unwrap();
        let buf = terminal.backend().buffer();
        assert!(has_cell(buf, "─", accent) || has_cell(buf, "│", accent));
        assert!(buffer_text(buf).contains("glorp"));
    }
    assert_overlay(render_help_overlay_for_test, p.accent.rgb);
    assert_overlay(render_evolution_overlay_for_test, p.accent.rgb);
    assert_overlay(render_hatch_overlay_for_test, p.accent.rgb);
}
```

- [ ] **Step 5: Run tests to confirm failure**

Run:

```bash
cargo test --test tui_render
```

Expected: FAIL with unresolved `tui` pieces.

- [ ] **Step 6: Implement the view model**

Add `src/tui/view_model.rs`:

- Include pet art, name, species/stage label, mood, age, XP progress, fed/happiness/energy bars.
- Include today effective tokens, recent 7-day sparkline values, source breakdown, current 10-minute bucket, recent event/feed lines, helper status, and errors.
- Include the most recent evolution event, if any, so `status` and watch mode can surface stage changes without relying on the live animation frame.
- Provide `WatchViewModel::fixture()` and `WatchViewModel::fixture_with_events()` for tests. The event fixture must include one usage line, one diagnostic line, and one evolution line with timestamp `13:42`, so style tests verify all semantic log rails.
- Do not include prompt/session browser, cost dashboard, PR/commit/diff stats, manual feed buttons, tweak controls, or litter selection.

- [ ] **Step 7: Implement layout**

Add `src/tui/layout.rs`:

- Wide layout: side-by-side left pet/vitals and right activity/status.
- Compact layout under 72 columns: vertical sections. At exactly 72 columns, keep the side-by-side layout; at 71 columns, switch to compact.
- Use `src/tui/style.rs` for every color and semantic style; no ad hoc `Color::Yellow`, `Color::Green`, or unrelated palette choices in layout/render code.
- Preserve the mockup's full restrained terminal feel: `surface` chrome over `bg`, dim centered title, faint/dashed section dividers, amber section accents, 20-cell `█`/`░` bars, muted timestamps, colored event rails, warm overlays, and compact labels.
- Add a small render helper for the mock terminal chrome: red/yellow/green dots when width allows, then `glorp -- <pet>@claude:~ -- <width>x<height>` in `dim`.
- Render recent activity with a single-cell left rail `▏` colored by event kind, a `faint` timestamp column, and `dim` normal text. The rail color must be `good` for usage, `accent` for evolution/help, and `bad` for diagnostics.
- Render recent 7-day activity as sparkline glyphs `▁▂▃▄▅▆▇█`; past days use `faint`, today uses `accent`.
- Add overlay render helpers for tests:

```rust
pub fn render_help_overlay_for_test(frame: &mut ratatui::Frame<'_>);
pub fn render_evolution_overlay_for_test(frame: &mut ratatui::Frame<'_>);
pub fn render_hatch_overlay_for_test(frame: &mut ratatui::Frame<'_>);
```

Each overlay uses `bg`/`surface`, an `accent` title/border, compact terminal copy, and no separate visual language.

- [ ] **Step 8: Implement app loop**

Add `src/tui/app.rs` and implement:

- Terminal setup/restore through `crossterm`.
- Terminal restore must run through a guard/drop path so raw mode, alternate screen, cursor visibility, and mouse capture are restored on normal quit and on command error.
- Animation tick around 250ms.
- Usage poll interval of 60 seconds, with the interval injectable in tests.
- Metabolism bucket window of 10 minutes.
- Immediate refresh key `r`.
- `q` quits.
- `?` opens help.
- Optional `p` affection adds happiness only.
- Provider errors stay in the UI without crashing.
- `WatchTestHarness` and `run_single_watch_tick_for_test` for integration tests. The harness should use an in-memory fake provider, exercise the same metabolism/bucket update path as watch mode, and avoid entering raw terminal mode.

- [ ] **Step 9: Wire `glorp watch` command**

Modify `src/commands/watch.rs`:

- Load state or instruct the user to run `glorp init`.
- Open usage DB.
- Build `CcusageCommandProvider`.
- Reconcile missed usage coarsely on open.
- Start TUI app loop.
- Save state after poll, decay, and quit.
- Keep a deterministic test entrypoint that runs a single render/poll tick without entering raw terminal mode.

- [ ] **Step 10: Run TUI tests**

Run:

```bash
cargo test --test style_tokens
cargo test --test tui_render
```

Expected: PASS.

- [ ] **Step 11: Commit**

```bash
git add src/tui src/commands/watch.rs src/lib.rs tests/style_tokens.rs tests/tui_render.rs
git commit -m "feat: add glorp watch tui"
```

## Task 10: Status, Doctor, Help, And Friendly Errors

**Stories:** `story-009`

**Files:**
- Create: `tests/doctor_status.rs`
- Modify: `src/commands/status.rs`
- Modify: `src/commands/doctor.rs`
- Modify: `src/commands/mod.rs`
- Modify: `src/lib.rs`
- Modify: `src/cli.rs`

- [ ] **Step 1: Write status and doctor tests**

Add `tests/doctor_status.rs`:

```rust
use assert_cmd::Command;
use glorp::storage::state::{write_state_for_test, PetStateFixture};
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn status_is_pipe_friendly_when_pet_exists() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("mochi"))
        .stdout(predicate::str::contains("effective tokens"))
        .stdout(predicate::str::contains("provider"))
        .stdout(predicate::str::contains("billing").not());
}

#[test]
fn doctor_reports_missing_helpers_with_setup_instructions() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env_remove("GLORP_CCUSAGE_BIN")
        .env_remove("GLORP_CCUSAGE_CODEX_BIN")
        .env("PATH", "/bin")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("ccusage"))
        .stdout(predicate::str::contains("not found"))
        .stdout(predicate::str::contains("npm install -g glorp"));
}

#[test]
fn diagnostics_do_not_print_raw_transcript_content() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", "tests/fixtures/helpers/ccusage-prompts.mjs")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("secret prompt").not())
        .stdout(predicate::str::contains("secret response").not())
        .stdout(predicate::str::contains("secret tool payload").not());
}

#[test]
fn doctor_sanitizes_invalid_json_and_helper_stderr() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", "tests/fixtures/helpers/ccusage-invalid-json.mjs")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("invalid_json"))
        .stdout(predicate::str::contains("secret prompt").not());

    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .env("GLORP_CCUSAGE_BIN", "tests/fixtures/helpers/ccusage-secret-stderr.mjs")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("helper_exit"))
        .stdout(predicate::str::contains("secret response").not());
}

#[test]
fn repeated_provider_failures_keep_last_known_pet_state() {
    let dir = tempdir().unwrap();
    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .args(["init", "--seed", "mochi-7f3a", "--name", "mochi"])
        .assert()
        .success();

    for _ in 0..2 {
        Command::cargo_bin("glorp").unwrap()
            .env("GLORP_CONFIG_DIR", dir.path())
            .env("GLORP_CCUSAGE_BIN", "tests/fixtures/helpers/ccusage-fails.mjs")
            .arg("status")
            .assert()
            .success()
            .stdout(predicate::str::contains("mochi"))
            .stdout(predicate::str::contains("helper_exit").or(predicate::str::contains("blocked")));
    }

    let state = std::fs::read_to_string(dir.path().join("state.json")).unwrap();
    assert!(state.contains("mochi"));
}

#[test]
fn status_includes_recent_evolution_event_when_present() {
    let dir = tempdir().unwrap();
    write_state_for_test(
        dir.path(),
        PetStateFixture::named("mochi")
            .with_stage("s3")
            .with_recent_event("evolved from sprout to bytebuddy"),
    )
    .unwrap();

    Command::cargo_bin("glorp").unwrap()
        .env("GLORP_CONFIG_DIR", dir.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("mochi"))
        .stdout(predicate::str::contains("evolved from sprout to bytebuddy"));
}
```

- [ ] **Step 2: Run tests to confirm failure**

Run:

```bash
cargo test --test doctor_status
```

Expected: FAIL until commands print the required surfaces.

- [ ] **Step 3: Implement status**

Add a small `#[doc(hidden)]` fixture helper in `src/storage/state.rs` for integration tests:

```rust
#[doc(hidden)]
pub struct PetStateFixture { /* build from real PetState fields */ }

#[doc(hidden)]
pub fn write_state_for_test(dir: &std::path::Path, fixture: PetStateFixture) -> crate::error::Result<()> {
    let paths = crate::paths::AppPaths::from_config_dir(dir.to_path_buf());
    StateStore::new(paths.state_file).save(&fixture.into_state())
}
```

The helper must use the real `PetState` serialization path rather than hand-writing JSON, so `status_includes_recent_evolution_event_when_present` fails if the state schema changes incompatibly.

`glorp status` prints:

- Pet name, species label, stage label, mood, age.
- XP progress within current stage.
- Fed/happiness/energy.
- Today's and recent effective-token totals.
- Provider health summary.
- Last diagnostic if present.
- Most recent evolution event if one is recorded.
- Local-derived/estimated label when applicable.

It exits successfully when a pet exists even if providers are blocked.

- [ ] **Step 4: Implement doctor**

`glorp doctor` prints:

- Config directory, `state.json`, `usage.sqlite`, and whether each is readable.
- Helper discovery result for env-provided paths and PATH fallback.
- Helper versions when `--version` succeeds.
- A parse probe for each helper using JSON mode.
- Recent stored provider diagnostics.
- Cursor health, including malformed cursor rows or decreasing cumulative totals, reported as sanitized diagnostic codes.
- Safe setup text:

```text
No usage helper was found.
Install the npm package with bundled helpers:
  npm install -g glorp
Or install helpers yourself and make sure these commands are on PATH:
  ccusage
  ccusage-codex
```

Doctor must not print prompt, response, tool payload, copied transcript fields, raw helper stderr, raw helper stdout, or JSON parse excerpts.

- [ ] **Step 5: Make help document MVP commands and keys**

`glorp help` and Clap help must list:

- `glorp init`
- `glorp watch`
- `glorp status`
- `glorp rename <name>`
- `glorp reset --yes`
- `glorp doctor`
- TUI keys: `q`, `?`, `r`, optional `p`

It must not list a normal-mode `feed`, `ship`, `treat`, `graveyard`, or `revive` command.

- [ ] **Step 6: Run command tests**

Run:

```bash
cargo test --test cli_smoke
cargo test --test doctor_status
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/commands src/cli.rs src/lib.rs tests/doctor_status.rs tests/cli_smoke.rs
git commit -m "feat: add status doctor and friendly diagnostics"
```

## Task 11: npm Distribution For The Rust Binary

**Stories:** `story-010`

**Files:**
- Create: `package.json`
- Create: `npm/glorp/package.json`
- Create: `npm/glorp/bin/glorp.js`
- Create: `npm/glorp/test/smoke.mjs`
- Create: `npm/platform/darwin-arm64/package.json`
- Create: `npm/platform/darwin-x64/package.json`
- Create: `npm/platform/linux-x64/package.json`
- Create: `npm/platform/linux-arm64/package.json`
- Create: `npm/platform/win32-x64/package.json`
- Create: `scripts/build-platform-package.mjs`

- [ ] **Step 1: Add npm package smoke test**

Create `npm/glorp/test/smoke.mjs`:

```javascript
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import { fileURLToPath } from "node:url";
import path from "node:path";

const here = path.dirname(fileURLToPath(import.meta.url));
const bin = path.resolve(here, "../bin/glorp.js");
const repoRoot = path.resolve(here, "../../..");

const help = spawnSync(process.execPath, [bin, "help"], { encoding: "utf8" });
assert.equal(help.status, 0, help.stderr);
assert.match(help.stdout, /watch/);
assert.doesNotMatch(help.stdout, /\bfeed\b/);

const doctor = spawnSync(process.execPath, [bin, "doctor"], { encoding: "utf8", env: { ...process.env, GLORP_CONFIG_DIR: path.join(here, ".tmp-config") } });
assert.equal(doctor.status, 0, doctor.stderr);
assert.match(doctor.stdout, /ccusage|helper/i);
assert.doesNotMatch(doctor.stdout, /secret prompt|secret response|tool payload/);

const tempBin = fs.mkdtempSync(path.join(os.tmpdir(), "glorp-path-helper-"));
const helper = path.join(repoRoot, "tests/fixtures/helpers/ccusage-ok.mjs");
const pathShim = path.join(tempBin, process.platform === "win32" ? "ccusage.cmd" : "ccusage");
if (process.platform === "win32") {
  fs.writeFileSync(pathShim, `@"${process.execPath}" "${helper}" %*\r\n`);
} else {
  fs.writeFileSync(pathShim, `#!/bin/sh\nexec "${process.execPath}" "${helper}" "$@"\n`);
  fs.chmodSync(pathShim, 0o755);
}

const fallback = spawnSync(process.execPath, [bin, "doctor"], {
  encoding: "utf8",
  env: {
    ...process.env,
    GLORP_CONFIG_DIR: path.join(here, ".tmp-config-path-fallback"),
    GLORP_SKIP_BUNDLED_HELPERS_FOR_TEST: "1",
    PATH: tempBin
  }
});
assert.equal(fallback.status, 0, fallback.stderr);
assert.match(fallback.stdout, /ccusage/i);
assert.doesNotMatch(fallback.stdout, /not found/i);

const missing = spawnSync(process.execPath, [bin, "doctor"], {
  encoding: "utf8",
  env: {
    ...process.env,
    GLORP_CONFIG_DIR: path.join(here, ".tmp-config-missing"),
    GLORP_SKIP_BUNDLED_HELPERS_FOR_TEST: "1",
    PATH: fs.mkdtempSync(path.join(os.tmpdir(), "glorp-empty-path-"))
  }
});
assert.equal(missing.status, 0, missing.stderr);
assert.match(missing.stdout, /not found|No usage helper|blocked/i);
```

- [ ] **Step 2: Create root npm workspace manifest**

Create `package.json`:

```json
{
  "private": true,
  "scripts": {
    "test": "cargo test && npm --workspace glorp test",
    "build": "cargo build --release",
    "package:smoke": "npm --workspace glorp test"
  },
  "workspaces": [
    "npm/glorp",
    "npm/platform/*"
  ]
}
```

- [ ] **Step 3: Create package manifest**

Create `npm/glorp/package.json`:

```json
{
  "name": "glorp",
  "version": "0.1.0",
  "description": "A terminal pet fed by real AI coding token usage",
  "license": "MIT",
  "bin": {
    "glorp": "bin/glorp.js"
  },
  "type": "module",
  "dependencies": {
    "@ccusage/codex": "^18.0.11",
    "ccusage": "^18.0.11"
  },
  "optionalDependencies": {
    "@glorp/darwin-arm64": "0.1.0",
    "@glorp/darwin-x64": "0.1.0",
    "@glorp/linux-x64": "0.1.0",
    "@glorp/linux-arm64": "0.1.0",
    "@glorp/win32-x64": "0.1.0"
  },
  "scripts": {
    "test": "node test/smoke.mjs"
  },
  "files": [
    "bin/glorp.js"
  ]
}
```

- [ ] **Step 4: Implement JS launcher**

Create `npm/glorp/bin/glorp.js`:

```javascript
#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

function platformPackageName() {
  const arch = process.arch === "x64" ? "x64" : process.arch === "arm64" ? "arm64" : process.arch;
  if (process.platform === "darwin") return `@glorp/darwin-${arch}`;
  if (process.platform === "linux") return `@glorp/linux-${arch}`;
  if (process.platform === "win32") return `@glorp/win32-${arch}`;
  throw new Error(`unsupported platform: ${process.platform}-${process.arch}`);
}

function resolveNativeBinary() {
  const pkg = platformPackageName();
  const pkgJson = require.resolve(`${pkg}/package.json`);
  const dir = path.dirname(pkgJson);
  const exe = process.platform === "win32" ? "glorp.exe" : "glorp";
  const bin = path.join(dir, "bin", exe);
  if (!fs.existsSync(bin)) throw new Error(`native glorp binary missing at ${bin}`);
  return bin;
}

function resolvePackageBin(pkg, binName) {
  try {
    const pkgJsonPath = require.resolve(`${pkg}/package.json`);
    const pkgJson = JSON.parse(fs.readFileSync(pkgJsonPath, "utf8"));
    const rel = typeof pkgJson.bin === "string" ? pkgJson.bin : pkgJson.bin?.[binName];
    if (!rel) return undefined;
    return path.resolve(path.dirname(pkgJsonPath), rel);
  } catch {
    return undefined;
  }
}

const env = { ...process.env };
if (env.GLORP_SKIP_BUNDLED_HELPERS_FOR_TEST !== "1") {
  env.GLORP_CCUSAGE_BIN ??= resolvePackageBin("ccusage", "ccusage");
  env.GLORP_CCUSAGE_CODEX_BIN ??= resolvePackageBin("@ccusage/codex", "ccusage-codex");
}
env.GLORP_NODE_BIN ??= process.execPath;

let native;
try {
  native = resolveNativeBinary();
} catch (err) {
  console.error(`glorp: ${err.message}`);
  process.exit(1);
}

const child = spawnSync(native, process.argv.slice(2), { stdio: "inherit", env });
if (child.error) {
  console.error(`glorp: ${child.error.message}`);
  process.exit(1);
}
process.exit(child.status ?? 1);
```

- [ ] **Step 5: Add platform package manifests**

For each platform directory, create a package manifest. Example for `npm/platform/darwin-arm64/package.json`:

```json
{
  "name": "@glorp/darwin-arm64",
  "version": "0.1.0",
  "license": "MIT",
  "os": ["darwin"],
  "cpu": ["arm64"],
  "files": ["bin/glorp"]
}
```

Use equivalent `os`, `cpu`, and executable path for `darwin-x64`, `linux-x64`, `linux-arm64`, and `win32-x64` with `bin/glorp.exe`.

- [ ] **Step 6: Add platform package build script**

Create `scripts/build-platform-package.mjs` to copy `target/release/glorp` into the current platform package's `bin/` directory during local smoke builds. It should:

- Accept `--platform current` and compute `darwin-arm64`, `darwin-x64`, `linux-x64`, `linux-arm64`, or `win32-x64`.
- When also passed `--print-package-name`, print the matching package name such as `@glorp/darwin-arm64` and exit without copying.
- Copy `target/release/glorp` to `npm/platform/<platform>/bin/glorp` or `target/release/glorp.exe` to `npm/platform/win32-x64/bin/glorp.exe`.
- Set executable mode `755` for non-Windows package binaries.
- Fail with a clear message if `cargo build --release` has not produced the binary.
- Verify every optional dependency in `npm/glorp/package.json` has a matching local `npm/platform/*/package.json` package name and version.
- Leave cross-platform artifact production to CI/release jobs; local smoke only needs the current platform binary.

- [ ] **Step 7: Run package smoke**

Run:

```bash
cargo build --release
node scripts/build-platform-package.mjs --platform current
npm install --workspaces --include-workspace-root
npm --workspace glorp test
npm pack --workspace glorp --dry-run
npm pack --workspace "$(node scripts/build-platform-package.mjs --platform current --print-package-name)" --dry-run
```

Expected: PASS. `npm pack --dry-run` includes `bin/glorp.js`; the current platform package contains the Rust binary; users do not need Rust at runtime.

- [ ] **Step 8: Commit**

```bash
git add package.json npm scripts
git commit -m "feat: package glorp rust binary for npm"
```

## Task 12: Documentation, Acceptance Audit, And Final Verification

**Stories:** all stories.

**Files:**
- Create: `README.md`
- Create: `docs/superpowers/build-report.yaml`
- Modify: any tests that need clearer fixture names after implementation

- [ ] **Step 1: Write README**

Create `README.md` with:

- What Glorp is: a terminal-native pet fed by real Claude Code and Codex token usage.
- Privacy promise: local-only, no telemetry, no prompt/response/tool payload storage.
- Install:

```bash
npm install -g glorp
glorp init
glorp watch
```

- Source installs:

```bash
cargo install --path .
glorp doctor
```

- MVP commands and TUI keys.
- Clear note that cost is local-derived display metadata and provider billing remains the source of truth.

- [ ] **Step 2: Write build report**

Create `docs/superpowers/build-report.yaml`:

```yaml
stories:
  story-001:
    status: completed
    evidence:
      - cargo test --test usage_provider
  story-002:
    status: completed
    evidence:
      - cargo test --test storage_privacy
  story-003:
    status: completed
    evidence:
      - cargo test --test generation
      - cargo test --test cli_smoke
  story-004:
    status: completed
    evidence:
      - cargo test --test game_rules effective_tokens
  story-005:
    status: completed
    evidence:
      - cargo test --test game_rules calibration evolution
  story-006:
    status: completed
    evidence:
      - cargo test --test game_rules metabolism
  story-007:
    status: completed
    evidence:
      - cargo test --test style_tokens
      - cargo test --test tui_render
  story-008:
    status: completed
    evidence:
      - cargo test --test generation render
      - cargo test --test style_tokens
  story-009:
    status: completed
    evidence:
      - cargo test --test doctor_status
  story-010:
    status: completed
    evidence:
      - node scripts/build-platform-package.mjs --platform current
      - npm --workspace glorp test
      - npm pack --workspace glorp --dry-run
      - npm pack --workspace "$(node scripts/build-platform-package.mjs --platform current --print-package-name)" --dry-run
```

- [ ] **Step 3: Run forbidden-feature audit**

Run:

```bash
rg -n "feed|ship|treat|graveyard|revive|death|permadeath|litter|stageOverride|speciesOverride|tweak" src tests npm README.md
```

Expected:

- No CLI command named `feed`, `ship`, `treat`, `graveyard`, or `revive`.
- No `death`, `graveyard`, `revive`, or `permadeath` model state.
- Any `feed` wording appears only as product prose like "fed by real usage" or event-log labels for real usage.
- No tweak, litter, species override, or stage override implementation.

- [ ] **Step 4: Run full verification**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
node scripts/build-platform-package.mjs --platform current
npm install --workspaces --include-workspace-root
npm --workspace glorp test
npm pack --workspace glorp --dry-run
npm pack --workspace "$(node scripts/build-platform-package.mjs --platform current --print-package-name)" --dry-run
```

Expected: all commands pass.

- [ ] **Step 5: Manual smoke with isolated config**

Run:

```bash
tmpdir="$(mktemp -d)"
GLORP_CONFIG_DIR="$tmpdir" cargo run -- init --seed mochi-7f3a --name mochi
GLORP_CONFIG_DIR="$tmpdir" cargo run -- status
GLORP_CONFIG_DIR="$tmpdir" cargo run -- doctor
```

Expected:

- Init says `mochi has hatched`.
- Status shows name, stage, mood, vitals, XP progress, effective-token summary, and provider health.
- Doctor shows config paths and helper status without raw transcript content.

- [ ] **Step 6: Commit**

```bash
git add README.md docs/superpowers/build-report.yaml
git commit -m "docs: document glorp completion and verification"
```

## Story Coverage Matrix

| Story | Covered By | Completion Gate |
| --- | --- | --- |
| `story-001` Usage Provider Through ccusage | Task 3, Task 10, Task 11 | `cargo test --test usage_provider`; doctor helper checks |
| `story-002` Local Persistence | Task 2, Task 5, Task 12 | `cargo test --test storage_privacy`; isolated config smoke |
| `story-003` Init And Generated Pet | Task 5, Task 8 | `cargo test --test generation`; `cargo test --test cli_smoke` |
| `story-004` Effective Token Model | Task 4 | `cargo test --test game_rules effective_tokens` |
| `story-005` Calibration And Evolution | Task 6, Task 9 | `cargo test --test game_rules calibration evolution`; TUI transition render |
| `story-006` Mood Decay And Wilted State | Task 7, Task 8, Task 9 | `cargo test --test game_rules metabolism`; forbidden-feature audit |
| `story-007` Watch Mode TUI Shell | Task 9 | `cargo test --test style_tokens`; `cargo test --test tui_render`; manual watch smoke |
| `story-008` Pet Renderer And Animation | Task 8, Task 9 | renderer determinism, style-token, and compact layout tests |
| `story-009` Status Doctor And Friendly Errors | Task 10, Task 12 | `cargo test --test doctor_status`; isolated config smoke |
| `story-010` npm Distribution For Rust Glorp | Task 11 | `npm --workspace glorp test`; glorp and platform `npm pack --dry-run` |

## Self-Review Checklist For The Implementer

- [ ] Every story has a passing automated test listed in `docs/superpowers/build-report.yaml`.
- [ ] `glorp init` starts at stage 0 even with historical usage available.
- [ ] Historical usage calibrates baseline only; it does not grant initial food, XP, or evolution.
- [ ] Cache reads use the configured low weight and never count 1:1.
- [ ] Cost is display-only and never changes food, mood, XP, or evolution.
- [ ] Provider failures produce diagnostics and a blocked-but-alive UI.
- [ ] `state.json` and `usage.sqlite` contain no raw prompts, responses, tool payloads, copied transcripts, or source transcript archives.
- [ ] TUI styles use `src/tui/style.rs` Tokenpet palette tokens; no unrelated color palette or ad hoc semantic color choices are introduced.
- [ ] Watch mode restores the terminal after quit and after provider errors.
- [ ] No manual food, fake ship, treat, tweak, litter, death, graveyard, revive, or permadeath mechanic exists in MVP code.
- [ ] Npm install path passes helper env vars to Rust and Rust still falls back to PATH discovery.
