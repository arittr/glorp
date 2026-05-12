#!/usr/bin/env node
// Bump Glorp's release version surfaces: Cargo.toml, Cargo.lock,
// npm/glorp/package.json, package-lock.json, all five platform packages,
// and the matching entries in glorp's optionalDependencies. Run before
// tagging a release so the publish workflow can assert tag == package
// version.
//
//   node scripts/bump-npm-version.mjs 0.2.0
//
// This script does not commit or tag — it just edits the files.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const defaultRepoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function fail(message) {
  console.error(message);
  process.exit(1);
}

function parseArgs(argv) {
  let repoRoot = defaultRepoRoot;
  let target;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--repo-root") {
      repoRoot = path.resolve(argv[++i] ?? "");
    } else if (!target) {
      target = arg;
    } else {
      fail("usage: node scripts/bump-npm-version.mjs [--repo-root <path>] <semver>");
    }
  }

  if (!target || !/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(target)) {
    fail("usage: node scripts/bump-npm-version.mjs [--repo-root <path>] <semver>");
  }

  return { repoRoot, target };
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function writeJson(file, value) {
  fs.writeFileSync(file, JSON.stringify(value, null, 2) + "\n");
}

function bumpCargoToml(file, target) {
  const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);
  let inPackage = false;
  let updated = false;

  const next = lines.map((line) => {
    const trimmed = line.trim();
    if (trimmed === "[package]") {
      inPackage = true;
      return line;
    }
    if (inPackage && trimmed.startsWith("[") && trimmed.endsWith("]")) {
      inPackage = false;
    }
    if (inPackage && /^version\s*=/.test(trimmed)) {
      updated = true;
      return `${line.match(/^\s*/)[0]}version = "${target}"`;
    }
    return line;
  });

  if (!updated) fail(`missing package version in ${file}`);
  fs.writeFileSync(file, next.join("\n"));
}

function bumpCargoLock(file, target) {
  const text = fs.readFileSync(file, "utf8");
  const blocks = text.split(/\n(?=\[\[package\]\]\n)/);
  let updated = false;

  const next = blocks.map((block) => {
    if (!/^name\s*=\s*"glorp"\s*$/m.test(block)) return block;
    updated = true;
    return block.replace(/^version\s*=\s*"[^"]+"\s*$/m, `version = "${target}"`);
  });

  if (!updated) fail(`missing glorp package in ${file}`);
  fs.writeFileSync(file, next.join("\n"));
}

function bumpPackageLock(file, target) {
  if (!fs.existsSync(file)) return;

  const lockfile = readJson(file);
  const main = lockfile.packages?.["npm/glorp"];
  if (!main) fail(`missing npm/glorp package entry in ${file}`);

  main.version = target;
  for (const name of Object.keys(main.optionalDependencies ?? {})) {
    main.optionalDependencies[name] = target;
  }
  writeJson(file, lockfile);
}

const { repoRoot, target } = parseArgs(process.argv.slice(2));

bumpCargoToml(path.join(repoRoot, "Cargo.toml"), target);
console.log(`bumped Cargo.toml → ${target}`);

bumpCargoLock(path.join(repoRoot, "Cargo.lock"), target);
console.log(`bumped Cargo.lock → ${target}`);

const mainPath = path.join(repoRoot, "npm/glorp/package.json");
const main = readJson(mainPath);
main.version = target;
for (const name of Object.keys(main.optionalDependencies ?? {})) {
  main.optionalDependencies[name] = target;
}
writeJson(mainPath, main);
console.log(`bumped ${path.relative(repoRoot, mainPath)} → ${target}`);

const platformRoot = path.join(repoRoot, "npm/platform");
for (const dir of fs.readdirSync(platformRoot)) {
  const file = path.join(platformRoot, dir, "package.json");
  if (!fs.existsSync(file)) continue;
  const pkg = readJson(file);
  pkg.version = target;
  writeJson(file, pkg);
  console.log(`bumped ${path.relative(repoRoot, file)} → ${target}`);
}

bumpPackageLock(path.join(repoRoot, "package-lock.json"), target);
console.log(`bumped package-lock.json → ${target}`);
