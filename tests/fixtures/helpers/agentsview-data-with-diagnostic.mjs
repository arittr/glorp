#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const fixtures = path.resolve(here, "..");
const args = process.argv.slice(2);

if (args.includes("--version")) {
  console.log("agentsview v0.32.1 (test fixture)");
  process.exit(0);
}

if (!args.includes("usage") || !args.includes("daily") || !args.includes("--json")) {
  console.error("unexpected agentsview argv");
  process.exit(2);
}
if (!args.includes("--timezone") || args[args.indexOf("--timezone") + 1] !== "America/Los_Angeles") {
  console.error("missing timezone");
  process.exit(2);
}

// Returns valid daily records plus one malformed model breakdown that triggers
// a benign diagnostic in the parser. Used to test that cursors are advanced
// even when diagnostics are present (bolus-prevention must not be skipped).
process.stdout.write(fs.readFileSync(path.join(fixtures, "agentsview-data-with-diagnostic.json"), "utf8"));
