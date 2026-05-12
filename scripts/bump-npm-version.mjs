#!/usr/bin/env node
// Bump the version in npm/glorp/package.json, all five platform packages,
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

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const target = process.argv[2];

if (!target || !/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(target)) {
  console.error("usage: node scripts/bump-npm-version.mjs <semver>");
  process.exit(1);
}

const mainPath = path.join(repoRoot, "npm/glorp/package.json");
const main = JSON.parse(fs.readFileSync(mainPath, "utf8"));
main.version = target;
for (const name of Object.keys(main.optionalDependencies ?? {})) {
  main.optionalDependencies[name] = target;
}
fs.writeFileSync(mainPath, JSON.stringify(main, null, 2) + "\n");
console.log(`bumped ${path.relative(repoRoot, mainPath)} → ${target}`);

const platformRoot = path.join(repoRoot, "npm/platform");
for (const dir of fs.readdirSync(platformRoot)) {
  const file = path.join(platformRoot, dir, "package.json");
  if (!fs.existsSync(file)) continue;
  const pkg = JSON.parse(fs.readFileSync(file, "utf8"));
  pkg.version = target;
  fs.writeFileSync(file, JSON.stringify(pkg, null, 2) + "\n");
  console.log(`bumped ${path.relative(repoRoot, file)} → ${target}`);
}
