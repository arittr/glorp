import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

function fail(scriptName, message) {
  console.error(`${scriptName}: ${message}`);
  process.exit(1);
}

export function buildMacosApp({
  mode,
  bundleIdentifier,
  bundledBinaryName,
  subcommand,
  lsuiElement,
  out,
  profile,
}) {
  const scriptName = `build-macos-${mode}-app`;

  if (process.platform !== "darwin") {
    fail(
      scriptName,
      `only supported on darwin; current platform is ${process.platform}`,
    );
  }

  if (profile !== "release" && profile !== "debug") {
    fail(scriptName, `--profile must be release or debug; got ${profile}`);
  }

  const outAppPath = out
    ? path.resolve(out)
    : path.join(repoRoot, "target", "macos", "Glorp.app");
  const targetDir = path.join(repoRoot, "target", profile);
  const binaryPath = path.join(targetDir, "glorp");

  const cargoArgs = ["build", "--bin", "glorp"];
  if (profile === "release") cargoArgs.push("--release");
  console.log(`${scriptName}: cargo ${cargoArgs.join(" ")}`);
  execFileSync("cargo", cargoArgs, { cwd: repoRoot, stdio: "inherit" });

  if (!fs.existsSync(binaryPath)) {
    fail(scriptName, `built binary missing at ${binaryPath}`);
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

  const bundledBinaryPath = path.join(macosDir, bundledBinaryName);
  fs.copyFileSync(binaryPath, bundledBinaryPath);
  fs.chmodSync(bundledBinaryPath, 0o755);

  const launcherPath = path.join(macosDir, "Glorp");
  fs.writeFileSync(
    launcherPath,
    `#!/bin/sh\nexec "$(dirname "$0")/${bundledBinaryName}" ${subcommand} "$@"\n`,
    { mode: 0o755 },
  );

  const lsuiElementPlist = lsuiElement
    ? "  <key>LSUIElement</key><true/>\n"
    : "";

  const infoPlist = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Glorp</string>
  <key>CFBundleDisplayName</key><string>Glorp</string>
  <key>CFBundleIdentifier</key><string>${bundleIdentifier}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleExecutable</key><string>Glorp</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
${lsuiElementPlist}  <key>NSHumanReadableCopyright</key><string>MIT-licensed. See LICENSE.</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
`;
  fs.writeFileSync(path.join(contentsDir, "Info.plist"), infoPlist);

  console.log(`${scriptName}: wrote ${outAppPath}`);
  console.log(`  exec: open '${outAppPath}'`);
  console.log(
    `  install: cp -R '${outAppPath}' /Applications && open /Applications/Glorp.app`,
  );
}
