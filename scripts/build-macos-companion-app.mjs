#!/usr/bin/env node
// Bundles the `glorp` binary into a minimal `Glorp.app` so the companion
// subcommand can launch as a regular macOS Dock app (double-clickable from
// /Applications, visible in the Dock and app switcher).
//
// Usage:
//   node scripts/build-macos-companion-app.mjs                 # release build, out: target/macos/Glorp.app
//   node scripts/build-macos-companion-app.mjs --out path.app  # custom output path
//   node scripts/build-macos-companion-app.mjs --profile debug # use the debug binary
//
// macOS-only. Errors out cleanly on other platforms.

import { buildMacosApp } from "./build-macos-app-shared.mjs";

function parseArgs(argv) {
  let out;
  let profile = "release";
  let binary;
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--out") {
      out = argv[++i];
    } else if (arg === "--profile") {
      profile = argv[++i];
    } else if (arg === "--binary") {
      binary = argv[++i];
    } else {
      console.error(`build-macos-companion-app: unknown argument ${arg}`);
      process.exit(1);
    }
  }
  return { out, profile, binary };
}

const { out, profile, binary } = parseArgs(process.argv.slice(2));

buildMacosApp({
  mode: "companion",
  bundleIdentifier: "dev.glorp.companion",
  bundledBinaryName: "glorp-companion",
  subcommand: "companion-app",
  lsuiElement: false,
  out,
  profile,
  binary,
});
