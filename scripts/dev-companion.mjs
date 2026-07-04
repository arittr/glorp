#!/usr/bin/env node
// Compatibility shim for the Rust xtask companion runner.
//
//   npm run companion            # debug build (fast), rebuild + restart
//   npm run companion -- --release
//
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
execFileSync(
  "cargo",
  ["xtask", "companion", "fresh", ...process.argv.slice(2)],
  {
    cwd: repoRoot,
    stdio: "inherit",
  },
);
