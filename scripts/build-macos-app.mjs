#!/usr/bin/env node
// Bundles the `glorp` binary into a minimal `Glorp.app` so the menubar
// subcommand can launch as a proper macOS LSUIElement (no dock icon, can
// be added to Login Items, double-clickable from /Applications).
//
// Usage:
//   node scripts/build-macos-app.mjs                 # release build, out: target/macos/Glorp.app
//   node scripts/build-macos-app.mjs --out path.app  # custom output path
//   node scripts/build-macos-app.mjs --profile debug # use the debug binary
//
// macOS-only. Errors out cleanly on other platforms.

import { buildMacosApp } from "./build-macos-app-shared.mjs";

function parseArgs(argv) {
  let out;
  let profile = "release";
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--out") {
      out = argv[++i];
    } else if (arg === "--profile") {
      profile = argv[++i];
    } else {
      console.error(`build-macos-app: unknown argument ${arg}`);
      process.exit(1);
    }
  }
  return { out, profile };
}

const { out, profile } = parseArgs(process.argv.slice(2));

buildMacosApp({
  mode: "menubar",
  bundleIdentifier: "dev.glorp.menubar",
  bundledBinaryName: "glorp-menubar",
  subcommand: "menubar",
  lsuiElement: true,
  out,
  profile,
});
