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

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function fail(message) {
  console.error(`build-macos-app: ${message}`);
  process.exit(1);
}

if (process.platform !== "darwin") {
  fail(`only supported on darwin; current platform is ${process.platform}`);
}

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
      fail(`unknown argument ${arg}`);
    }
  }
  if (profile !== "release" && profile !== "debug") {
    fail(`--profile must be release or debug; got ${profile}`);
  }
  return { out, profile };
}

const { out, profile } = parseArgs(process.argv.slice(2));
const outAppPath = out
  ? path.resolve(out)
  : path.join(repoRoot, "target", "macos", "Glorp.app");
const targetDir = path.join(repoRoot, "target", profile);
const binaryPath = path.join(targetDir, "glorp");

const cargoArgs = ["build", "--bin", "glorp"];
if (profile === "release") cargoArgs.push("--release");
console.log(`build-macos-app: cargo ${cargoArgs.join(" ")}`);
execFileSync("cargo", cargoArgs, { cwd: repoRoot, stdio: "inherit" });

if (!fs.existsSync(binaryPath)) {
  fail(`built binary missing at ${binaryPath}`);
}

const pkgJson = JSON.parse(
  fs.readFileSync(path.join(repoRoot, "package.json"), "utf8"),
);
const version = pkgJson.version || "0.0.0";

const contentsDir = path.join(outAppPath, "Contents");
const macosDir = path.join(contentsDir, "MacOS");
const resourcesDir = path.join(contentsDir, "Resources");

fs.rmSync(outAppPath, { recursive: true, force: true });
fs.mkdirSync(macosDir, { recursive: true });
fs.mkdirSync(resourcesDir, { recursive: true });

// The .app's executable. We name it `glorp-menubar` and have it `exec` the
// real binary in menubar mode, so launching the .app always opens the
// menubar UI without requiring users to know the subcommand.
const bundledBinaryName = "glorp-menubar";
const bundledBinaryPath = path.join(macosDir, bundledBinaryName);
fs.copyFileSync(binaryPath, bundledBinaryPath);
fs.chmodSync(bundledBinaryPath, 0o755);

// Launcher shim: small shell script that execs the bundled binary with the
// `menubar` subcommand. Lets the .app double-click run the menubar mode
// directly while keeping `glorp <other-subcommand>` available via the
// unwrapped binary on PATH.
const launcherPath = path.join(macosDir, "Glorp");
fs.writeFileSync(
  launcherPath,
  `#!/bin/sh\nexec "$(dirname "$0")/${bundledBinaryName}" menubar "$@"\n`,
  { mode: 0o755 },
);

const infoPlist = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Glorp</string>
  <key>CFBundleDisplayName</key><string>Glorp</string>
  <key>CFBundleIdentifier</key><string>dev.glorp.menubar</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleExecutable</key><string>Glorp</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHumanReadableCopyright</key><string>MIT-licensed. See LICENSE.</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
`;
fs.writeFileSync(path.join(contentsDir, "Info.plist"), infoPlist);

console.log(`build-macos-app: wrote ${outAppPath}`);
console.log(`  exec: open '${outAppPath}'`);
console.log(
  `  install: cp -R '${outAppPath}' /Applications && open /Applications/Glorp.app`,
);
