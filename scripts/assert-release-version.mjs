#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const DEFAULT_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function fail(message) {
  console.error(message);
  process.exit(1);
}

function parseArgs(argv) {
  let repoRoot = DEFAULT_ROOT;
  let tag;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--repo-root") {
      repoRoot = path.resolve(argv[++i] ?? "");
    } else if (arg === "--tag") {
      tag = argv[++i];
    } else {
      fail(`usage: node scripts/assert-release-version.mjs [--repo-root <path>] [--tag vX.Y.Z]`);
    }
  }

  return { repoRoot, tag };
}

function readText(file) {
  return fs.readFileSync(file, "utf8");
}

function readJson(file) {
  return JSON.parse(readText(file));
}

function rel(repoRoot, file) {
  return path.relative(repoRoot, file).replaceAll(path.sep, "/");
}

function releaseVersionFromTag(tag) {
  if (!tag) return undefined;
  const match = /^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/.exec(tag);
  if (!match) fail(`release tag ${tag} must look like vX.Y.Z`);
  return match[1];
}

function readCargoTomlVersion(file) {
  const lines = readText(file).split(/\r?\n/);
  const start = lines.findIndex((line) => line.trim() === "[package]");
  if (start === -1) fail(`missing [package] section in ${file}`);

  for (const line of lines.slice(start + 1)) {
    const trimmed = line.trim();
    if (trimmed.startsWith("[") && trimmed.endsWith("]")) break;

    const version = /^version\s*=\s*"([^"]+)"\s*$/.exec(trimmed);
    if (version) return version[1];
  }

  fail(`missing package version in ${file}`);
}

function readCargoLockPackageVersion(file, name) {
  const text = readText(file);
  for (const block of text.split(/\n(?=\[\[package\]\]\n)/)) {
    const packageName = /^name\s*=\s*"([^"]+)"\s*$/m.exec(block)?.[1];
    if (packageName !== name) continue;
    const version = /^version\s*=\s*"([^"]+)"\s*$/m.exec(block)?.[1];
    if (!version) fail(`missing ${name} version in ${file}`);
    return version;
  }
  fail(`missing ${name} package in ${file}`);
}

function listPlatformPackageFiles(repoRoot) {
  const platformRoot = path.join(repoRoot, "npm/platform");
  return fs
    .readdirSync(platformRoot)
    .map((dir) => path.join(platformRoot, dir, "package.json"))
    .filter((file) => fs.existsSync(file))
    .sort();
}

function main() {
  const { repoRoot, tag } = parseArgs(process.argv.slice(2));
  const failures = [];

  const mainPackagePath = path.join(repoRoot, "npm/glorp/package.json");
  const mainPackage = readJson(mainPackagePath);
  const expected = releaseVersionFromTag(tag) ?? mainPackage.version;

  function expectVersion(label, actual) {
    if (actual !== expected) {
      failures.push(`${label} version ${actual} does not match expected ${expected}`);
    }
  }

  expectVersion("Cargo.toml", readCargoTomlVersion(path.join(repoRoot, "Cargo.toml")));
  expectVersion(
    "Cargo.lock glorp package",
    readCargoLockPackageVersion(path.join(repoRoot, "Cargo.lock"), "glorp")
  );
  expectVersion("npm/glorp/package.json", mainPackage.version);

  const optionalDependencies = mainPackage.optionalDependencies ?? {};
  const localPlatformPackages = new Map();
  for (const file of listPlatformPackageFiles(repoRoot)) {
    const pkg = readJson(file);
    localPlatformPackages.set(pkg.name, { version: pkg.version, file });
    expectVersion(rel(repoRoot, file), pkg.version);
  }

  for (const [name, version] of Object.entries(optionalDependencies)) {
    expectVersion(`npm/glorp optional dependency ${name}`, version);
    if (!localPlatformPackages.has(name)) {
      failures.push(`npm/glorp optional dependency ${name} has no matching local platform package`);
    }
  }

  for (const [name, local] of localPlatformPackages) {
    if (!Object.hasOwn(optionalDependencies, name)) {
      failures.push(`${rel(repoRoot, local.file)} package ${name} is missing from optionalDependencies`);
    }
  }

  const lockfilePath = path.join(repoRoot, "package-lock.json");
  if (fs.existsSync(lockfilePath)) {
    const lockfile = readJson(lockfilePath);
    const lockedMain = lockfile.packages?.["npm/glorp"];
    if (!lockedMain) {
      failures.push("package-lock.json is missing npm/glorp package entry");
    } else {
      expectVersion("package-lock.json npm/glorp", lockedMain.version);
      for (const [name, version] of Object.entries(lockedMain.optionalDependencies ?? {})) {
        expectVersion(`package-lock.json optional dependency ${name}`, version);
      }
    }
  }

  if (failures.length > 0) {
    console.error(["release version check failed:", ...failures.map((failure) => `- ${failure}`)].join("\n"));
    process.exit(1);
  }

  console.log(`release versions ok: ${expected}`);
}

main();
