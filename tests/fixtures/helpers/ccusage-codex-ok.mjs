#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
if (args[0] === "--version") {
  console.log("ccusage-codex 18.0.11");
  process.exit(0);
}
if (
  args[0] === "daily" &&
  args.includes("--json") &&
  args.includes("--offline") &&
  !args.includes("--order")
) {
  process.stdout.write(fs.readFileSync(path.join(here, "..", "ccusage-codex-daily.json"), "utf8"));
  process.exit(0);
}
console.error(`unsupported ccusage-codex fixture args: ${args.join(" ")}`);
process.exit(2);
