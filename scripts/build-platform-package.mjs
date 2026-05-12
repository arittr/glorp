#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const supported = new Set(["darwin-arm64", "darwin-x64", "linux-arm64", "linux-x64", "win32-x64"]);

function fail(message) {
  console.error(`build-platform-package: ${message}`);
  process.exit(1);
}

function currentPlatform() {
  const arch = process.arch === "x64" ? "x64" : process.arch === "arm64" ? "arm64" : process.arch;
  const key = `${process.platform}-${arch}`;
  if (!supported.has(key)) fail(`unsupported current platform ${process.platform}-${process.arch}`);
  return key;
}

function parseArgs(argv) {
  let platform;
  let printPackageName = false;
  let printPackageDir = false;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--platform") {
      platform = argv[++i];
    } else if (arg === "--print-package-name") {
      printPackageName = true;
    } else if (arg === "--print-package-dir") {
      printPackageDir = true;
    } else {
      fail(`unknown argument ${arg}`);
    }
  }

  if (platform === "current") platform = currentPlatform();
  if (!platform) fail("missing --platform current");
  if (!supported.has(platform)) fail(`unsupported platform ${platform}`);
  return { platform, printPackageName, printPackageDir };
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function verifyOptionalPackages() {
  const glorpPackage = readJson(path.join(repoRoot, "npm/glorp/package.json"));
  const optional = glorpPackage.optionalDependencies ?? {};
  const platformRoot = path.join(repoRoot, "npm/platform");
  const localPackages = new Map();

  for (const dir of fs.readdirSync(platformRoot)) {
    const pkgFile = path.join(platformRoot, dir, "package.json");
    if (!fs.existsSync(pkgFile)) continue;
    const pkg = readJson(pkgFile);
    localPackages.set(pkg.name, { version: pkg.version, dir });
  }

  for (const [name, version] of Object.entries(optional)) {
    const local = localPackages.get(name);
    if (!local) fail(`optional dependency ${name} has no matching npm/platform package`);
    if (local.version !== version) {
      fail(`optional dependency ${name}@${version} does not match local version ${local.version}`);
    }
  }

  for (const name of localPackages.keys()) {
    if (!Object.hasOwn(optional, name)) fail(`local platform package ${name} is missing from optionalDependencies`);
  }
}

function packageName(platform) {
  return `@arittr/glorp-${platform}`;
}

const { platform, printPackageName, printPackageDir } = parseArgs(process.argv.slice(2));
verifyOptionalPackages();

if (printPackageName) {
  console.log(packageName(platform));
  process.exit(0);
}

if (printPackageDir) {
  console.log(path.join(repoRoot, "npm/platform", platform));
  process.exit(0);
}

const isWindows = platform.startsWith("win32-");
const exe = isWindows ? "glorp.exe" : "glorp";
const source = path.join(repoRoot, "target/release", exe);
if (!fs.existsSync(source)) {
  fail(`missing release binary ${source}; run \`npm run build\` first`);
}

const destDir = path.join(repoRoot, "npm/platform", platform, "bin");
const dest = path.join(destDir, exe);
fs.mkdirSync(destDir, { recursive: true });
fs.copyFileSync(source, dest);
if (!isWindows) fs.chmodSync(dest, 0o755);
console.log(`copied ${path.relative(repoRoot, source)} to ${path.relative(repoRoot, dest)}`);
