#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const fixtures = path.resolve(here, "..");

if (process.argv.includes("--version")) {
  console.log("agentsview v0.32.1 (test fixture)");
  process.exit(0);
}

process.stdout.write(fs.readFileSync(path.join(fixtures, "agentsview-malformed-number.json"), "utf8"));
