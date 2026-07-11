#!/usr/bin/env node
// Bundles the `glorp` binary into a minimal `Glorp.app` so the companion
// subcommand can launch as a regular macOS Dock app (double-clickable from
// /Applications, visible in the Dock and app switcher).
//
// Usage:
//   node scripts/build-macos-companion-app.mjs                 # release build, out: target/macos/Glorp.app
//   node scripts/build-macos-companion-app.mjs --out path.app  # custom output path
//   node scripts/build-macos-companion-app.mjs --profile debug # use the debug binary
//   node scripts/build-macos-companion-app.mjs --features a,b   # override cargo features
//
// Locally, arm64 macOS compiles `retained-renderer` by default; pass
// `--features` to override. `--binary` bundles a prebuilt artifact without
// rebuilding, so features do not apply there.
//
// macOS-only. Errors out cleanly on other platforms.

import { buildMacosApp } from "./build-macos-app-shared.mjs";

// Apple Silicon ships the retained renderer backend; Intel is Smooth-only. A
// local companion build therefore compiles `retained-renderer` on arm64 macOS
// and no extra feature elsewhere, unless the caller overrides with `--features`.
function defaultCompanionFeatures() {
  if (process.platform === "darwin" && process.arch === "arm64") {
    return ["retained-renderer"];
  }
  return [];
}

function parseArgs(argv) {
  let out;
  let profile = "release";
  let binary;
  let features;
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--out") {
      out = argv[++i];
    } else if (arg === "--profile") {
      profile = argv[++i];
    } else if (arg === "--binary") {
      binary = argv[++i];
    } else if (arg === "--features") {
      features = argv[++i].split(",").filter(Boolean);
    } else {
      console.error(`build-macos-companion-app: unknown argument ${arg}`);
      process.exit(1);
    }
  }
  return { out, profile, binary, features: features ?? defaultCompanionFeatures() };
}

const { out, profile, binary, features } = parseArgs(process.argv.slice(2));

buildMacosApp({
  mode: "companion",
  bundleIdentifier: "dev.glorp.companion",
  bundledBinaryName: "glorp-companion",
  subcommand: "companion-app",
  lsuiElement: false,
  out,
  profile,
  binary,
  features,
});
