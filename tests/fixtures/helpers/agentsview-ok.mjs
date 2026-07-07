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

const agent = args[args.indexOf("--agent") + 1];
if (!args.includes("usage") || !args.includes("daily") || !args.includes("--json")) {
  console.error("unexpected agentsview argv");
  process.exit(2);
}
if (!args.includes("--timezone") || args[args.indexOf("--timezone") + 1] !== "America/Los_Angeles") {
  console.error("missing timezone");
  process.exit(2);
}
if (!args.includes("--since")) {
  console.error("missing since");
  process.exit(2);
}
const since = args[args.indexOf("--since") + 1];
if (since !== "1970-01-01") {
  if (!args.includes("--until") || since !== "2026-06-17" || args[args.indexOf("--until") + 1] !== "2026-06-18") {
    console.error("missing recent-day window");
    process.exit(2);
  }
}

const file = agent === "claude" ? "agentsview-claude-daily.json" : "agentsview-codex-daily.json";
process.stdout.write(fs.readFileSync(path.join(fixtures, file), "utf8"));
