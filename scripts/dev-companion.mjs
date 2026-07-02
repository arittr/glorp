#!/usr/bin/env node
// Build the macOS companion bundle and relaunch it fresh in one step.
//
//   npm run companion            # debug build (fast), rebuild + restart
//   npm run companion -- --release
//
// `open` alone only foregrounds an already-running app, so we quit any running
// instance first and then reopen the freshly built bundle.
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const profile = process.argv.includes("--release") ? "release" : "debug";
const app = path.join(repoRoot, "target/macos/Glorp.app");

const run = (cmd, args, opts = {}) =>
  execFileSync(cmd, args, { cwd: repoRoot, stdio: "inherit", ...opts });

console.log(`dev-companion: building bundle (${profile})…`);
run("node", ["scripts/build-macos-companion-app.mjs", "--profile", profile]);

// Quit a running instance so the reopen picks up the new build. Both calls are
// best-effort — nothing may be running yet.
const quiet = { stdio: "ignore" };
try {
  run("osascript", ["-e", 'quit app "Glorp"'], quiet);
} catch {}
try {
  run("pkill", ["-f", "Glorp.app/Contents/MacOS"], quiet);
} catch {}
// Give the OS a moment to release the quitting instance before reopening.
try {
  run("sleep", ["1"], quiet);
} catch {}

console.log("dev-companion: relaunching…");
run("open", [app]);
console.log(`dev-companion: up (${profile}). Bundle: ${app}`);
